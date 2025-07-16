#![allow(clippy::module_name_repetitions)]

use crate::{
    arch::x86_64::structures::gdt::{GlobalDescriptorTable, SystemSegmentDescriptor},
    mem::alloc::KERNEL_ALLOCATOR,
};

type StackTableStack = crate::mem::stack::Stack<0x16000>;

// Pre-defined indexes into the interrupt stack table (IST).
#[repr(u16)]
#[derive(Debug, IntoPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum InterruptStackTableIndex {
    Debug = 0,
    NonMaskableInterrupt = 1,
    DoubleFault = 2,
    MachineCheck = 3,
}

#[repr(C, packed(4))]
pub struct TaskStateSegment {
    _1: [u8; 4],

    /// The full 64-bit canonical forms of the stack pointers (RSP) for privilege levels 0-2.
    /// The stack pointers used when a privilege level change occurs from a lower privilege level to a higher one.
    privilege_stack_table: [Option<StackTableStack>; 3],

    _2: [u8; 8],

    /// The full 64-bit canonical forms of the interrupt stack table (IST) pointers.
    /// The stack pointers used when an entry in the Interrupt Descriptor Table has an IST value other than 0.
    interrupt_stack_table: [Option<StackTableStack>; 7],

    _3: [u8; 10],

    /// The 16-bit offset to the I/O permission bit map from the 64-bit TSS base.
    iomap_base: u16,
}

impl TaskStateSegment {
    pub fn new_with_stacks() -> Self {
        fn allocate_stack_table_stack() -> StackTableStack {
            StackTableStack::allocate_new()
                .expect("failed to allocate a new stack for task state segment")
        }

        let mut tss = TaskStateSegment {
            privilege_stack_table: [Some(allocate_stack_table_stack()), None, None],
            interrupt_stack_table: [const { None }; _],
            iomap_base: u16::try_from(size_of::<TaskStateSegment>()).unwrap(),
            _1: [0u8; _],
            _2: [0u8; _],
            _3: [0u8; _],
        };

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
    /// Only one [`TaskStateSegment`] should need to be loaded on each hardware thread.
    /// It's likely a runtime error if more than one are loaded per hardware threads.
    pub fn load_local(self) {
        GlobalDescriptorTable::with_temporary(|temp_gdt| {
            let tss_ptr = KERNEL_ALLOCATOR
                .allocate_t::<Self>()
                .expect("could not allocate task state segment");

            // Safety: `self` is dereferenceable as `Self`.
            let tss_segment_descriptor = unsafe { SystemSegmentDescriptor::from_tss(tss_ptr) };
            let tss_segment_selector = temp_gdt.append_segment(tss_segment_descriptor);

            trace!("Loading: {tss_ptr:#X?}");

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
