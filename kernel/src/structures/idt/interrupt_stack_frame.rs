use crate::Address;

/// Wrapper type for the interrupt stack frame pushed by the CPU.
///
/// This helps to ensure no modifications of the ISF are made without
/// explicit consent, through `as_mut()`
#[repr(C)]
pub struct InterruptStackFrame(InterruptStackFrameValue);

impl InterruptStackFrame {
    /// Gives mutable access to the interrupt stack frame.
    ///
    /// ## Safety
    /// This function is unsafe since modifying the interrupt stack frame
    /// can potentially lead undefined behavior.
    pub unsafe fn as_mut(&mut self) -> &mut InterruptStackFrameValue {
        &mut self.0
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct InterruptStackFrameValue {
    pub instruction_pointer: Address,
    pub code_segment: u64,
    pub cpu_flags: u64,
    pub stack_pointer: Address,
    pub stack_segment: u64,
}

impl core::fmt::Debug for InterruptStackFrame {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl core::fmt::Debug for InterruptStackFrameValue {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("InterruptStackFrame")
            .field("Instruction Pointer", &self.instruction_pointer)
            .field("Code Segment", &self.cpu_flags)
            .field("Stack Pointer", &self.stack_pointer)
            .field("Stack Segment", &self.stack_segment)
            .finish()
    }
}
