use crate::arch::x86_64::{
    registers::ProcessorFlags,
    structures::gdt::{
        SegmentSelector, kcode_selector, kdata_selector, ucode_selector, udata_selector,
    },
};

/// Represents the interrupt stack frame pushed by the CPU on interrupt or
/// exception entry.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InterruptStackFrame {
    /// The instruction pointer at the time of the interrupt.
    ///
    /// # Remarks
    ///
    /// This value points to the instruction that should be executed when the
    /// interrupt handler returns. For most interrupts, this value points to the
    /// instruction immediately following the last executed instruction.
    /// However, for some exceptions (e.g., page faults), this value points to
    /// the faulting instruction, so that the instruction is restarted on
    /// return. See the documentation of the
    /// [`InterruptDescriptorTable`][crate::arch::x86_64::structures::idt::InterruptDescriptorTable]
    /// fields for more details.
    pub instruction_address: usize,

    /// The code segment at the time of the interrupt.
    pub code_segment: SegmentSelector,

    _cs_padding: [u8; 6],

    /// The flags at the time of the interrupt.
    pub cpu_flags: usize,

    /// The stack pointer at the time of the interrupt.
    pub stack_address: usize,

    /// The stack segment at the time of the interrupt.
    pub stack_segment: SegmentSelector,

    _ss_padding: [u8; 6],
}

impl InterruptStackFrame {
    /// Constructs a new [`InterruptStackFrame`].
    pub fn new(
        instruction_address: usize,
        code_segment: SegmentSelector,
        cpu_flags: ProcessorFlags,
        stack_address: usize,
        stack_segment: SegmentSelector,
    ) -> Self {
        Self {
            instruction_address,
            code_segment,
            cpu_flags: cpu_flags.bits(),
            stack_address,
            stack_segment,

            _cs_padding: [0u8; _],
            _ss_padding: [0u8; _],
        }
    }

    pub fn new_kernel(instruction_address: usize, stack_address: usize) -> Self {
        Self::new(
            instruction_address,
            kcode_selector(),
            ProcessorFlags::INTERRUPT_FLAG,
            stack_address,
            kdata_selector(),
        )
    }

    pub fn new_user(instruction_address: usize, stack_address: usize) -> Self {
        Self::new(
            instruction_address,
            ucode_selector(),
            ProcessorFlags::INTERRUPT_FLAG,
            stack_address,
            udata_selector(),
        )
    }
}

#[allow(clippy::missing_fields_in_debug)]
impl core::fmt::Debug for InterruptStackFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Interrupt Stack Frame")
            .field("Instruction Pointer", &self.instruction_address)
            .field("Code Segment", &self.code_segment)
            .field("Stack Pointer", &self.stack_address)
            .field("Stack Segment", &self.stack_segment)
            .field("CPU Flags", &self.cpu_flags)
            .finish()
    }
}
