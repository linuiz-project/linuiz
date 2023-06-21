pub mod instructions;
pub mod registers;
pub mod structures;

pub mod cpuid {
    pub use raw_cpuid::*;
    use spin::Lazy;

    pub static CPUID: Lazy<CpuId<raw_cpuid::CpuIdReaderNative>> = Lazy::new(CpuId::new);
    pub static FEATURE_INFO: Lazy<FeatureInfo> = Lazy::new(|| CPUID.get_feature_info().expect("no CPUID.01H support"));
    pub static EXT_FEATURE_INFO: Lazy<Option<ExtendedFeatures>> = Lazy::new(|| CPUID.get_extended_feature_info());
    pub static EXT_FUNCTION_INFO: Lazy<Option<ExtendedProcessorFeatureIdentifiers>> =
        Lazy::new(|| CPUID.get_extended_processor_and_feature_identifiers());
    pub static VENDOR_INFO: Lazy<Option<VendorInfo>> = Lazy::new(|| CPUID.get_vendor_info());
}

/// Gets the ID of the current core.
#[allow(clippy::map_unwrap_or)]
pub fn get_cpu_id() -> u32 {
    use cpuid::{CPUID, FEATURE_INFO};

    CPUID
        // IA32 SDM instructs to enumerate this leaf first...
        .get_extended_topology_info_v2()
        // ... this leaf second ...
        .or_else(|| CPUID.get_extended_topology_info())
        .and_then(|mut iter| iter.next())
        .map(|info| info.x2apic_id())
        // ... and finally, this leaf as an absolute fallback.
        .unwrap_or_else(|| FEATURE_INFO.initial_local_apic_id().into())
}
