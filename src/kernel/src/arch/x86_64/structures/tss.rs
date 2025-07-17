#![allow(clippy::module_name_repetitions)]

use crate::{
    arch::x86_64::structures::gdt::{GlobalDescriptorTable, SystemSegmentDescriptor},
    mem::alloc::KERNEL_ALLOCATOR,
};
use core::ptr::NonNull;

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
#[derive(FromZeros)]
pub struct TaskStateSegment {
    _1: [u8; 4],

    /// The stack pointers used when a privilege level change occurs from a lower privilege level
    /// to a higher one (e.g. ring 3 to ring 0).
    privilege_stack_table: [Option<NonNull<StackTableStack>>; 3],

    _2: [u8; 8],

    /// The stack pointers used when an entry in the Interrupt Descriptor Table has an IST value
    /// other than 0.
    interrupt_stack_table: [Option<NonNull<StackTableStack>>; 7],

    _3: [u8; 10],

    /// The 16-bit offset to the I/O permission bit map from the 64-bit TSS base.
    iomap_base: u16,
}

impl TaskStateSegment {
    /// Loads this [`TaskStateSegment`] into the task state segment register.
    ///
    /// # Remarks
    ///
    /// Only one [`TaskStateSegment`] should be loaded on each hardware thread. It's likely a
    /// runtime error if more than one are loaded per hardware threads.
    pub fn load_local() {
        fn allocate_stack_table_stack() -> NonNull<StackTableStack> {
            KERNEL_ALLOCATOR
                .allocate_t::<StackTableStack>()
                .expect("failed to allocate a new stack for task state segment")
        }

        let tss = crate::mem::alloc::KERNEL_ALLOCATOR
            .allocate_t_static::<Self>()
            .expect("failed to allocate task state segment");

        // Set the stack for transitions to ring 0.
        tss.privilege_stack_table[0] = Some(allocate_stack_table_stack());

        // Set the stacks for faults that cannot be disabled or are caused by runtime errors.
        tss.interrupt_stack_table[usize::from(u16::from(InterruptStackTableIndex::Debug))] =
            Some(allocate_stack_table_stack());
        tss.interrupt_stack_table
            [usize::from(u16::from(InterruptStackTableIndex::NonMaskableInterrupt))] =
            Some(allocate_stack_table_stack());
        tss.interrupt_stack_table[usize::from(u16::from(InterruptStackTableIndex::DoubleFault))] =
            Some(allocate_stack_table_stack());
        tss.interrupt_stack_table[usize::from(u16::from(InterruptStackTableIndex::MachineCheck))] =
            Some(allocate_stack_table_stack());

        GlobalDescriptorTable::with_temporary(|temp_gdt| {
            let tss_segment_descriptor = SystemSegmentDescriptor::from_tss(tss);
            let tss_segment_selector = temp_gdt.append_segment(tss_segment_descriptor);

            trace!("Loading: {:#X?}", core::ptr::from_ref(tss));

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
