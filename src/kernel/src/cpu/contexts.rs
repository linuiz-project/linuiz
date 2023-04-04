#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ControlContext {
    pub ip: u64,
    pub sp: u64,
}

#[cfg(target_arch = "x86_64")]
pub use x64_Context::*;
#[cfg(target_arch = "x86_64")]
mod x64_Context {
    use crate::arch::x64::registers;

    pub type SyscallContext = registers::PreservedRegistersSysv64;

    pub struct ArchContext {
        gprs: registers::GeneralPurpose,
        state: registers::Stateful,
    }

    impl ArchContext {
        pub const fn kernel_context() -> Self {
            Self {
                gprs: registers::GeneralPurpose::default(),
                state: registers::Stateful::kernel_state(registers::RFlags::INTERRUPT_FLAG),
            }
        }

        pub const fn user_context() -> Self {
            Self {
                gprs: registers::GeneralPurpose::default(),
                state: registers::Stateful::user_state(registers::RFlags::INTERRUPT_FLAG),
            }
        }
    }
}
