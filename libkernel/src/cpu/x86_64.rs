pub mod cpuid {
    pub use raw_cpuid::*;

    lazy_static::lazy_static! {}
}

lazy_static::lazy_static! {
    pub static ref CPUID: cpuid::CpuId = cpuid::CpuId::new();
    pub static ref FEATURE_INFO: Option<cpuid::FeatureInfo> = CPUID.get_feature_info();
    pub static ref EXT_FEATURE_INFO: Option<cpuid::ExtendedFeatures> = CPUID.get_extended_feature_info();
    pub static ref EXT_FUNCTION_INFO: Option<cpuid::ExtendedProcessorFeatureIdentifiers> = CPUID.get_extended_processor_and_feature_identifiers();
}

/// Reads [`crate::regisers::x86_64::msr::IA32_APIC_BASE`] to determine whether the current core
/// is the bootstrap processor.
#[inline(always)]
pub fn is_bsp() -> bool {
    crate::registers::x86_64::msr::IA32_APIC_BASE::get_is_bsp()
}
