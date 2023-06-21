#[cfg(target_arch = "x86_64")]
mod context_impl {
    use libsys::{Address, Virtual};

    use crate::arch::x86_64::{registers::RFlags, structures::gdt};

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

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct State {
        pub ip: Address<Virtual>,
        pub cs: usize,
        pub rfl: RFlags,
        pub sp: Address<Virtual>,
        pub ss: usize,
    }

    impl State {
        pub fn kernel(ip: Address<Virtual>, sp: Address<Virtual>) -> Self {
            Self {
                ip,
                cs: gdt::kernel_code_selector().0.into(),
                rfl: RFlags::INTERRUPT_FLAG,
                sp,
                ss: gdt::kernel_data_selector().0.into(),
            }
        }

        pub fn user(ip: Address<Virtual>, sp: Address<Virtual>) -> Self {
            Self {
                ip,
                cs: gdt::user_code_selector().0.into(),
                rfl: RFlags::INTERRUPT_FLAG,
                sp,
                ss: gdt::user_data_selector().0.into(),
            }
        }
    }
}

pub use context_impl::*;
