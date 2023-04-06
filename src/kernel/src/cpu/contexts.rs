use core::ptr::+;

pub use _x86_64::*;

use crate::{arch::x64::registers::RFlags, proc::task::EntryPoint};
#[cfg(target_arch = "x86_64")]
mod _x86_64 {
    use crate::arch::x64::{registers::RFlags, structures::gdt};

    #[repr(C)]
    #[derive(Debug, Default, Clone)]
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
    }

    #[derive(Debug, Clone)]
    pub struct State {
        pub cs: u64,
        pub ss: u64,
        pub flags: RFlags,
    }

    impl State {
        pub fn kernel(flags: RFlags) -> Self {
            Self { cs: gdt::kernel_code_selector().0 as u64, ss: gdt::kernel_data_selector().0 as u64, flags }
        }

        pub fn user(flags: RFlags) -> Self {
            Self { cs: gdt::user_code_selector().0 as u64, ss: gdt::user_data_selector().0 as u64, flags }
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Control {
    pub ip: u64,
    pub sp: u64,
}

pub enum ContextType {
    Syscall { rax: u64, rdi: u64, rsi: u64, rdx: u64, r10: u64, r8: u64, r9: u64 },
    Arch { gprs: Registers, state: State },
}

impl ContextType {
    pub const fn kernel_default() -> Self {
        Self::Arch { gprs: Registers::default(), state: State::kernel(RFlags::INTERRUPT_FLAG) }
    }

    pub const fn user_default() -> Self {
        Self::Arch { gprs: Registers::default(), state: State::user(RFlags::INTERRUPT_FLAG) }
    }
}

pub struct Context {
    control: Control,
    ty: ContextType,
}

impl Context {
    pub fn new_kernel(ip: EntryPoint, sp: NonNull<u8>) -> Self {
        Context {
            control: Control { ip: ip.0 as u64, sp: sp.as_ptr() as u64 },
            ty: ContextType::Arch(ArchContext::kernel_default()),
        }
    }

    pub fn new_user(ip: EntryPoint, sp: NonNull<u8>) -> Self {
        Context {
            control: Control { ip: ip.0 as u64, sp: sp.as_ptr() as u64 },
            ty: ContextType::Arch(ArchContext::user_default()),
        }
    }
}
