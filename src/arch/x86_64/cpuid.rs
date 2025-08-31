use raw_cpuid::{
    ApmInfo, CpuId, CpuIdReaderNative, ExtendedFeatures, ExtendedProcessorFeatureIdentifiers,
    ExtendedTopologyIter, FeatureInfo, HypervisorInfo, ProcessorFrequencyInfo, VendorInfo,
};

fn cpuid() -> CpuId<CpuIdReaderNative> {
    CpuId::new()
}

pub fn vendor_info() -> Option<VendorInfo> {
    cpuid().get_vendor_info()
}

pub fn feature_info() -> Option<FeatureInfo> {
    cpuid().get_feature_info()
}

pub fn extended_feature_info() -> Option<ExtendedFeatures> {
    cpuid().get_extended_feature_info()
}

pub fn extended_feature_identifiers() -> Option<ExtendedProcessorFeatureIdentifiers> {
    cpuid().get_extended_processor_and_feature_identifiers()
}

pub fn processor_frequency_info() -> Option<ProcessorFrequencyInfo> {
    cpuid().get_processor_frequency_info()
}

pub fn advanced_power_management_info() -> Option<ApmInfo> {
    cpuid().get_advanced_power_mgmt_info()
}

pub fn get_extended_topology_info() -> Option<ExtendedTopologyIter<CpuIdReaderNative>> {
    cpuid().get_extended_topology_info()
}

pub fn get_extended_topology_info_v2() -> Option<ExtendedTopologyIter<CpuIdReaderNative>> {
    cpuid().get_extended_topology_info_v2()
}

pub fn hypervisor_info() -> Option<HypervisorInfo<CpuIdReaderNative>> {
    cpuid().get_hypervisor_info()
}

pub fn print_info() {
    let vendor_info = vendor_info();

    info!(
        "CPU Vendor: {}",
        vendor_info.as_ref().map_or("UNKNOWN", VendorInfo::as_str)
    );
    debug!("{:#?}", feature_info());
    debug!("{:#?}", extended_feature_info());
    debug!("{:#?}", extended_feature_identifiers());
    debug!("{:#?}", processor_frequency_info());
    debug!("{:#?}", advanced_power_management_info());
    debug!("{:#?}", hypervisor_info());
}
