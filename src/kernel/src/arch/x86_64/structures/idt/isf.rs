use crate::arch::x86_64::{
    registers::RFlags,
    structures::gdt::{
        KCODE_SELECTOR, KDATA_SELECTOR, PrivilegeLevel, SegmentSelector, UCODE_SELECTOR,
        UDATA_SELECTOR,
    },
};
use libsys::{Address, Virtual};

/// Represents the interrupt stack frame pushed by the CPU on interrupt or exception entry.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InterruptStackFrame {
    instruction_pointer: u64,

    code_segment: u16,

    _1: [u8; 6],

    /// The flags register before the interrupt handler was invoked.
    cpu_flags: u64,

    /// The stack pointer at the time of the interrupt.
    stack_pointer: u64,

    /// The stack segment descriptor at the time of the interrupt (often zero in 64-bit mode).
    stack_segment: u16,

    _2: [u8; 6],
}

impl InterruptStackFrame {
    // TODO make unsafe? not sure if creating an invalid ISF is actually unsafe, since it may not always be used.
    /// Constructs a new [`InterruptStackFrame`].
    pub fn new(
        instruction_pointer: Address<Virtual>,
        code_segment: SegmentSelector,
        cpu_flags: RFlags,
        stack_pointer: Address<Virtual>,
        stack_segment: SegmentSelector,
    ) -> Self {
        Self {
            instruction_pointer: u64::try_from(instruction_pointer.get()).unwrap(),
            code_segment: code_segment.as_u16(),
            _1: [0u8; _],
            cpu_flags: cpu_flags.bits(),
            stack_pointer: u64::try_from(stack_pointer.get()).unwrap(),
            stack_segment: stack_segment.as_u16(),
            _2: [0u8; _],
        }
    }

    pub fn new_kernel(
        instruction_pointer: Address<Virtual>,
        stack_pointer: Address<Virtual>,
    ) -> Self {
        Self::new(
            instruction_pointer,
            *KCODE_SELECTOR.wait(),
            RFlags::INTERRUPT_FLAG,
            stack_pointer,
            *KDATA_SELECTOR.wait(),
        )
    }

    pub fn new_user(
        instruction_pointer: Address<Virtual>,
        stack_pointer: Address<Virtual>,
    ) -> Self {
        Self::new(
            instruction_pointer,
            *UCODE_SELECTOR.wait(),
            RFlags::INTERRUPT_FLAG,
            stack_pointer,
            *UDATA_SELECTOR.wait(),
        )
    }

    /// Gets the return instruction pointer.
    ///
    /// ## Remarks
    ///
    /// This value points to the instruction that should be executed when the interrupt
    /// handler returns. For most interrupts, this value points to the instruction immediately
    /// following the last executed instruction. However, for some exceptions (e.g., page faults),
    /// this value points to the faulting instruction, so that the instruction is restarted on
    /// return. See the documentation of the [`InterruptDescriptorTable`] fields for more details.
    pub fn get_instruction_pointer(&self) -> Address<Virtual> {
        Address::new(usize::try_from(self.instruction_pointer).unwrap()).unwrap()
    }

    /// Stores the new return instruction pointer.
    ///
    /// # Safety
    ///
    /// TODO
    pub unsafe fn set_instruction_pointer(&mut self, instruction_pointer: Address<Virtual>) {
        self.instruction_pointer = u64::try_from(instruction_pointer.get()).unwrap();
    }

    /// Get the return code segment selector.
    pub fn get_code_segment(&self) -> SegmentSelector {
        SegmentSelector::new(
            self.code_segment >> 3,
            PrivilegeLevel::try_from(self.code_segment & 0b11).unwrap(),
        )
    }

    /// Set the return code segment selector.
    ///
    /// # Safety
    ///
    /// TODO
    pub unsafe fn set_code_segment(&mut self, segment_selector: SegmentSelector) {
        self.code_segment = segment_selector.as_u16();
    }

    /// Get the return cpu flags.
    pub fn get_cpu_flags(&self) -> RFlags {
        RFlags::from_bits_truncate(self.cpu_flags)
    }

    /// Set the return cpu flags.
    ///
    /// # Safety
    ///
    /// TODO
    pub unsafe fn set_cpu_flags(&mut self, cpu_flags: RFlags) {
        self.cpu_flags = cpu_flags.bits();
    }

    /// Get the return stack pointer.
    pub fn get_stack_pointer(&self) -> Address<Virtual> {
        Address::new(usize::try_from(self.stack_pointer).unwrap()).unwrap()
    }

    /// Set the return stack pointer.
    ///
    /// # Safety
    ///
    /// TODO
    pub unsafe fn set_stack_pointer(&mut self, stack_pointer: Address<Virtual>) {
        self.stack_pointer = u64::try_from(stack_pointer.get()).unwrap();
    }

    /// Get the return stack segment selector.
    pub fn get_stack_segment(&self) -> SegmentSelector {
        SegmentSelector::new(
            self.stack_segment >> 3,
            PrivilegeLevel::try_from(self.stack_segment & 0b11).unwrap(),
        )
    }

    /// Set the return stack segment selector.
    ///
    /// # Safety
    ///
    /// TODO
    pub unsafe fn set_stack_segment(&mut self, segment_selector: SegmentSelector) {
        self.stack_segment = segment_selector.as_u16();
    }
}

impl core::fmt::Debug for InterruptStackFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InterruptStackFrame")
            .field("instruction_pointer", &self.get_instruction_pointer())
            .field("code_segment", &self.get_code_segment())
            .field("cpu_flags", &self.get_cpu_flags())
            .field("stack_pointer", &self.get_stack_pointer())
            .field("stack_segment", &self.get_stack_segment())
            .finish()
    }
}
