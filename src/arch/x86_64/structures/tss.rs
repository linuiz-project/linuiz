use crate::{
    arch::x86_64::structures::gdt::{GlobalDescriptorTable, SystemSegmentDescriptor},
    mem::alloc::allocate_kernel_stack,
};
use core::{num::NonZero, ptr::NonNull};
use num_enum::{FromPrimitive, IntoPrimitive};

#[cfg(debug_assertions)]
const PAGES_PER_STACK_TABLE_STACK: NonZero<usize> = NonZero::new(16).unwrap();
#[cfg(not(debug_assertions))]
const PAGES_PER_STACK_TABLE_STACK: NonZero<usize> = NonZero::new(4).unwrap();

// Pre-defined indexes into the interrupt stack table (IST).
#[repr(u16)]
#[derive(Debug, IntoPrimitive, FromPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum InterruptStackTableIndex {
    Debug = 0,
    NonMaskableInterrupt = 1,
    DoubleFault = 2,
    MachineCheck = 3,

    #[default]
    Unknown,
}

#[repr(C, packed(4))]
pub struct TaskStateSegment {
    _1: [u8; 4],

    /// The stack pointers used when a privilege level change occurs from a
    /// lower privilege level to a higher one (e.g. ring 3 to ring 0).
    privilege_stack_table: [Option<NonNull<u8>>; 3],

    _2: [u8; 8],

    /// The stack pointers used when an entry in the Interrupt Descriptor Table
    /// has an IST value other than 0.
    interrupt_stack_table: [Option<NonNull<u8>>; 7],

    _3: [u8; 10],

    /// The 16-bit offset to the I/O permission bit map from the 64-bit TSS
    /// base.
    iomap_base: u16,
}

const_assert!(size_of::<TaskStateSegment>() == 104);

impl TaskStateSegment {
    pub fn new() -> Self {
        fn allocate_stack_table_stack() -> NonNull<u8> {
            allocate_kernel_stack(PAGES_PER_STACK_TABLE_STACK)
                .expect("failed to allocate a task state segment stack")
        }

        let mut tss = Self {
            privilege_stack_table: [None; _],
            interrupt_stack_table: [None; _],
            iomap_base: u16::try_from(size_of::<Self>()).unwrap(),
            _1: [0u8; _],
            _2: [0u8; _],
            _3: [0u8; _],
        };

        // Set the stack for transitions to ring 0.
        tss.privilege_stack_table[0] = Some(allocate_stack_table_stack());

        // Set the stacks for faults that cannot be disabled or are caused by runtime
        // errors.
        tss.interrupt_stack_table[usize::from(u16::from(InterruptStackTableIndex::Debug))] =
            Some(allocate_stack_table_stack());
        tss.interrupt_stack_table
            [usize::from(u16::from(InterruptStackTableIndex::NonMaskableInterrupt))] =
            Some(allocate_stack_table_stack());
        tss.interrupt_stack_table[usize::from(u16::from(InterruptStackTableIndex::DoubleFault))] =
            Some(allocate_stack_table_stack());
        tss.interrupt_stack_table[usize::from(u16::from(InterruptStackTableIndex::MachineCheck))] =
            Some(allocate_stack_table_stack());

        tss
    }

    /// Loads this [`TaskStateSegment`] into the task state segment register.
    ///
    /// # Remarks
    ///
    /// It's likely a runtime error if more than one [`TaskStateSegment`]s are
    /// loaded per processor.
    pub fn load(&self) {
        GlobalDescriptorTable::with_temporary(|temp_gdt| {
            let tss_segment_descriptor = SystemSegmentDescriptor::from_tss(self);
            let tss_segment_selector = temp_gdt.append_segment(tss_segment_descriptor);

            // Load the temporary GDT for loading TSS.
            // Safety: Temporary GDT is identical to static GDT + 1 entry, so cannot
            //         cause undefined behaviour by loading.
            unsafe {
                temp_gdt.load();
            }

            trace!("Loading: {:#X?}", core::ptr::from_ref(self));

            // Safety: No memory safety concerns.
            unsafe {
                core::arch::asm!(
                    "ltr {:x}",
                    in(reg) tss_segment_selector.as_u16(),
                    options(nostack, nomem, preserves_flags)
                );
            }
        });
    }
}
