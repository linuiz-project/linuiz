pub mod cpuid {
    pub use raw_cpuid::*;
}

lazy_static::lazy_static! {
    pub static ref CPUID: cpuid::CpuId = cpuid::CpuId::new();
    pub static ref FEATURE_INFO: cpuid::FeatureInfo = CPUID.get_feature_info().expect("no CPUID.01H support");
    pub static ref EXT_FEATURE_INFO: Option<cpuid::ExtendedFeatures> = CPUID.get_extended_feature_info();
    pub static ref EXT_FUNCTION_INFO: Option<cpuid::ExtendedProcessorFeatureIdentifiers> = CPUID.get_extended_processor_and_feature_identifiers();
    pub static ref VENDOR_INFO: Option<cpuid::VendorInfo> = CPUID.get_vendor_info();
}

/// Reads [`crate::regisers::x86_64::msr::IA32_APIC_BASE`] to determine whether the current core
/// is the bootstrap processor.
#[inline(always)]
pub fn is_bsp() -> bool {
    crate::registers::msr::IA32_APIC_BASE::get_is_bsp()
}

/// Gets the vendor of the CPU.
pub fn get_vendor() -> Option<&'static str> {
    VENDOR_INFO.as_ref().map(|info| info.as_str())
}

/// Gets the ID of the current core.
pub fn get_id() -> u32 {
    CPUID
        // IA32 SDM instructs to enumerate this leaf first...
        .get_extended_topology_info_v2()
        // ... this leaf second ...
        .or_else(|| CPUID.get_extended_topology_info())
        .and_then(|mut iter| iter.next())
        .map(|info| info.x2apic_id())
        // ... and finally, this leaf as an absolute fallback.
        .unwrap_or_else(|| FEATURE_INFO.initial_local_apic_id() as u32)
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GeneralRegisters {
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

impl GeneralRegisters {
    pub const fn empty() -> Self {
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
