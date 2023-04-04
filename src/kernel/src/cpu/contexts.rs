#[repr(C, packed)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Control {
    pub ip: u64,
    pub sp: u64,
}

#[cfg(target_arch = "x86_64")]
pub use x64_Context::*;
#[cfg(target_arch = "x86_64")]
mod x64_Context {
    use crate::arch::x64::registers;
    use super::Control;

    pub type SyscallContext = registers::PreservedRegistersSysv64;

    pub struct ArchContext {
        gprs: registers::GeneralPurpose,
        state: registers::Stateful,
        control: Control,
    }

    impl ArchContext {
        pub const fn kernel_context() -> Self {
            Self {
                gprs: registers::GeneralPurpose::default(),
                state: registers::Stateful::kernel_state(registers::RFlags::INTERRUPT_FLAG),
                control: Control::default(),
            }
        }

        pub const fn user_context() -> Self {
            Self {
                gprs: registers::GeneralPurpose::default(),
                state: registers::Stateful::user_state(registers::RFlags::INTERRUPT_FLAG),
                control: Control::default(),
            }
        }
    }
}

pub enum Context {
    Syscall(SyscallContext),
    Arch(ArchContext),
}