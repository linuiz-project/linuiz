use crate::uptr;

#[cfg(target_arch = "x86_64")]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registers {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rfl: crate::arch::x64::registers::RFlags,
    pub cs: u64,
    pub ss: u64,
}

impl Registers {
    pub fn user_default() -> Self {
        Self {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rfl: crate::arch::x64::registers::RFlags::INTERRUPT_FLAG,
            cs: crate::arch::x64::structures::gdt::kernel_code_selector().0 as u64,
            ss: crate::arch::x64::structures::gdt::kernel_data_selector().0 as u64,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub ip: uptr,
    pub sp: uptr,
}
