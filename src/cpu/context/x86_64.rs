use crate::interrupts::syscall::SyscallResult;
use core::num::NonZero;

pub type Execution = crate::arch::x86_64::structures::idt::InterruptStackFrame;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Registers {
    pub rax: usize,
    pub rbx: usize,
    pub rcx: usize,
    pub rdx: usize,
    pub rdi: usize,
    pub rsi: usize,
    pub rbp: usize,
    pub r8: usize,
    pub r9: usize,
    pub r10: usize,
    pub r11: usize,
    pub r12: usize,
    pub r13: usize,
    pub r14: usize,
    pub r15: usize,
}

impl Registers {
    /// The register value associated with a system call vector.
    ///
    /// # Remark
    ///
    /// The value may or may not be a system call vector. It is the caller's
    /// responsibility to only use it as such where appropriate.
    pub fn syscall_vector(&self) -> usize {
        self.rsi
    }

    /// The register value associated with the first argument of a system call.
    ///
    /// # Remark
    ///
    /// The value may or may not be a system call argument. It is the caller's
    /// responsibility to only use it as such where appropriate.
    pub fn syscall_arg1(&self) -> usize {
        self.rdi
    }

    /// The register value associated with the second argument of a system call.
    ///
    /// # Remark
    ///
    /// The value may or may not be a system call argument. It is the caller's
    /// responsibility to only use it as such where appropriate.
    pub fn syscall_arg2(&self) -> usize {
        self.rax
    }

    /// The register value associated with the third argument of a system call.
    ///
    /// # Remark
    ///
    /// The value may or may not be a system call argument. It is the caller's
    /// responsibility to only use it as such where appropriate.
    pub fn syscall_arg3(&self) -> usize {
        self.rcx
    }

    /// The register value associated with the fourth argument of a system call.
    ///
    /// # Remark
    ///
    /// The value may or may not be a system call argument. It is the caller's
    /// responsibility to only use it as such where appropriate.
    pub fn syscall_arg4(&self) -> usize {
        self.rdx
    }

    pub fn set_syscall_result(&mut self, result: SyscallResult) {
        self.rdi = result.code.map_or(0, NonZero::get);
        self.rsi = result.value;
    }
}
