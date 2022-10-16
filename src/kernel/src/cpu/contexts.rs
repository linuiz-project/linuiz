#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ControlContext {
    pub ip: u64,
    pub sp: u64,
}

#[cfg(target_arch = "x86_64")]
pub type ArchContext = (crate::arch::x64::cpu::GeneralRegisters, crate::arch::x64::cpu::SpecialRegisters);
#[cfg(target_arch = "x86_64")]
pub type SyscallContext = crate::arch::x64::cpu::PreservedRegistersSysv64;
