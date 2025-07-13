use raw_cpuid::{
    ApmInfo, CpuId, CpuIdReaderNative, ExtendedFeatures, ExtendedProcessorFeatureIdentifiers,
    ExtendedTopologyIter, FeatureInfo, HypervisorInfo, ProcessorFrequencyInfo, VendorInfo,
};
use spin::Lazy;

static CPUID: Lazy<CpuId<CpuIdReaderNative>> = Lazy::new(CpuId::new);

pub fn vendor_info() -> &'static str {
    static VENDOR_INFO: Lazy<Option<VendorInfo>> = Lazy::new(|| CPUID.get_vendor_info());

    VENDOR_INFO
        .as_ref()
        .map_or("UNKNOWN", raw_cpuid::VendorInfo::as_str)
}

pub fn feature_info() -> Option<&'static FeatureInfo> {
    static FEATURE_INFO: Lazy<Option<FeatureInfo>> = Lazy::new(|| CPUID.get_feature_info());

    FEATURE_INFO.as_ref()
}

pub fn extended_feature_info() -> Option<&'static ExtendedFeatures> {
    static EXT_FEATURE_INFO: Lazy<Option<ExtendedFeatures>> =
        Lazy::new(|| CPUID.get_extended_feature_info());

    EXT_FEATURE_INFO.as_ref()
}

pub fn extended_feature_identifiers() -> Option<&'static ExtendedProcessorFeatureIdentifiers> {
    static EXT_FEATURE_IDENTIFIERS: Lazy<Option<ExtendedProcessorFeatureIdentifiers>> =
        Lazy::new(|| CPUID.get_extended_processor_and_feature_identifiers());

    EXT_FEATURE_IDENTIFIERS.as_ref()
}

pub fn processor_frequency_info() -> Option<&'static ProcessorFrequencyInfo> {
    static PROCESSOR_FREQUENCY_INFO: Lazy<Option<ProcessorFrequencyInfo>> =
        Lazy::new(|| CPUID.get_processor_frequency_info());

    PROCESSOR_FREQUENCY_INFO.as_ref()
}

pub fn advanced_power_management_info() -> Option<&'static ApmInfo> {
    static ADVANCED_PWM_INFO: Lazy<Option<ApmInfo>> =
        Lazy::new(|| CPUID.get_advanced_power_mgmt_info());

    ADVANCED_PWM_INFO.as_ref()
}

pub fn get_extended_topology_info() -> Option<&'static ExtendedTopologyIter<CpuIdReaderNative>> {
    static EXTENDED_TOPOLOGY_INFO: Lazy<Option<ExtendedTopologyIter<CpuIdReaderNative>>> =
        Lazy::new(|| CPUID.get_extended_topology_info());

    EXTENDED_TOPOLOGY_INFO.as_ref()
}

pub fn get_extended_topology_info_v2() -> Option<&'static ExtendedTopologyIter<CpuIdReaderNative>> {
    static EXTENDED_TOPOLOGY_INFO_V2: Lazy<Option<ExtendedTopologyIter<CpuIdReaderNative>>> =
        Lazy::new(|| CPUID.get_extended_topology_info_v2());

    EXTENDED_TOPOLOGY_INFO_V2.as_ref()
}

pub fn hypervisor_info() -> Option<&'static HypervisorInfo<CpuIdReaderNative>> {
    static HYPERVISOR_INFO: Lazy<Option<HypervisorInfo<CpuIdReaderNative>>> =
        Lazy::new(|| CPUID.get_hypervisor_info());

    HYPERVISOR_INFO.as_ref()
}

pub fn print_info() {
    info!("CPU Vendor: {}", vendor_info());
    debug!("{:#?}", feature_info());
    debug!("{:#?}", extended_feature_info());
    debug!("{:#?}", extended_feature_identifiers());
    debug!("{:#?}", processor_frequency_info());
    debug!("{:#?}", advanced_power_management_info());
    debug!("{:#?}", hypervisor_info());
}
