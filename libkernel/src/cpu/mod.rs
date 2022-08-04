pub mod x64;

lazy_static::lazy_static! {
    #[cfg(target_arch = "x86_64")]
    static ref VENDOR_INFO: Option<crate::cpu::x64::cpuid::VendorInfo> = crate::cpu::x64::CPUID.get_vendor_info();
}

/// Gets the vendor of the CPU.
pub fn get_vendor() -> Option<&'static str> {
    #[cfg(target_arch = "x86_64")]
    {
        VENDOR_INFO.as_ref().map(|info| info.as_str())
    }
}

/// Gets the ID of the current core.
pub fn get_id() -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        use crate::cpu::x64::{CPUID, FEATURE_INFO};

        CPUID
            // IA32 SDM instructs to enumerate this leaf first...
            .get_extended_topology_info_v2()
            // ... this leaf second ...
            .or_else(|| CPUID.get_extended_topology_info())
            .and_then(|mut iter| iter.next())
            .map(|info| info.x2apic_id())
            // ... and finally, this leaf as an absolute fallback.
            .or_else(|| FEATURE_INFO.as_ref().map(|info| info.initial_local_apic_id() as u32))
            .expect("CPU does not support any ID-reporting CPUID leaves")
    }
}

#[cfg(target_arch = "x86_64")]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ThreadRegisters {
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

impl ThreadRegisters {
    pub const fn empty() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
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
            }
        }
    }
}
