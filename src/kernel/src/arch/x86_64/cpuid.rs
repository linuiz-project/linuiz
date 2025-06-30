use raw_cpuid::{
    CpuId, CpuIdReaderNative, ExtendedFeatures, ExtendedProcessorFeatureIdentifiers, FeatureInfo,
    VendorInfo,
};
use spin::Lazy;

pub static CPUID: Lazy<CpuId<CpuIdReaderNative>> = Lazy::new(CpuId::new);

pub static VENDOR_INFO: Lazy<Option<VendorInfo>> = Lazy::new(|| CPUID.get_vendor_info());

pub static FEATURE_INFO: Lazy<FeatureInfo> =
    Lazy::new(|| CPUID.get_feature_info().expect("no CPUID.01H support"));

pub static EXT_FEATURE_INFO: Lazy<Option<ExtendedFeatures>> =
    Lazy::new(|| CPUID.get_extended_feature_info());

pub static EXT_FEATURE_IDENTIFIERS: Lazy<Option<ExtendedProcessorFeatureIdentifiers>> =
    Lazy::new(|| CPUID.get_extended_processor_and_feature_identifiers());
