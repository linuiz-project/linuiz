pub mod x86_64;

lazy_static::lazy_static! {
    #[cfg(target_arch = "x86_64")]
    static ref VENDOR_INFO: Option<crate::cpu::x86_64::cpuid::VendorInfo> = crate::cpu::x86_64::CPUID.get_vendor_info();
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
        use crate::cpu::x86_64::{CPUID, FEATURE_INFO};

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
