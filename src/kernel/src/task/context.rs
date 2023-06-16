#[cfg(target_arch = "x86_64")]
mod context_impl {

    use crate::arch::x64::{registers::RFlags, structures::gdt};

    #[repr(C)]
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct Registers {
        pub rax: u64,
        pub rbx: u64,
        pub rcx: u64,
        pub rdx: u64,
        pub rdi: u64,
        pub rsi: u64,
        pub rbp: u64,
        pub r8: u64,
        pub r9: u64,
        pub r10: u64,
        pub r11: u64,
        pub r12: u64,
        pub r13: u64,
        pub r14: u64,
        pub r15: u64,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct State {
        pub ip: u64,
        pub cs: u64,
        pub rfl: RFlags,
        pub sp: u64,
        pub ss: u64,
    }

    impl State {
        pub fn kernel(ip: u64, sp: u64) -> Self {
            Self {
                ip,
                sp,
                rfl: RFlags::INTERRUPT_FLAG,
                cs: gdt::kernel_code_selector().0.into(),
                ss: gdt::kernel_data_selector().0.into(),
            }
        }

        pub fn user(ip: u64, sp: u64) -> Self {
            Self {
                ip,
                sp,
                rfl: RFlags::INTERRUPT_FLAG,
                cs: gdt::user_code_selector().0.into(),
                ss: gdt::user_data_selector().0.into(),
            }
        }
    }
}

pub use context_impl::*;
