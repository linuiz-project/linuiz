pub mod instructions;
pub mod registers;
pub mod structures;

pub mod cpuid {
    use spin::Lazy;

    pub use raw_cpuid::*;

    pub static CPUID: Lazy<CpuId<CpuIdReaderNative>> = Lazy::new(CpuId::new);
    pub static FEATURE_INFO: Lazy<FeatureInfo> =
        Lazy::new(|| CPUID.get_feature_info().expect("no CPUID.01H support"));
    pub static EXT_FEATURE_INFO: Lazy<Option<ExtendedFeatures>> =
        Lazy::new(|| CPUID.get_extended_feature_info());
    pub static EXT_FUNCTION_INFO: Lazy<Option<ExtendedProcessorFeatureIdentifiers>> =
        Lazy::new(|| CPUID.get_extended_processor_and_feature_identifiers());
    pub static VENDOR_INFO: Lazy<Option<VendorInfo>> = Lazy::new(|| CPUID.get_vendor_info());
}

/// ## Safety
///
/// This function has the potential to modify CPU state in such a way as to disrupt
/// software execution. It should be run only once per hardware thread at the very
/// beginning of code execution.
pub unsafe fn configure_hwthread() {
    use registers::{
        control::{CR0, CR0Flags, CR4, CR4Flags},
        msr,
    };

    // Safety: This is the first and only time `CR0` will be set.
    unsafe {
        CR0::write(
            CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG,
        );
    }

    let mut cr4_flags = CR4Flags::PAE | CR4Flags::PGE | CR4Flags::OSXMMEXCPT;

    if cpuid::FEATURE_INFO.has_de() {
        cr4_flags.insert(CR4Flags::DE);
    }

    if cpuid::FEATURE_INFO.has_fxsave_fxstor() {
        cr4_flags.insert(CR4Flags::OSFXSR);
    }

    if cpuid::FEATURE_INFO.has_mce() {
        cr4_flags.insert(CR4Flags::MCE);
    }

    if cpuid::FEATURE_INFO.has_pcid() {
        cr4_flags.insert(CR4Flags::PCIDE);
    }

    if cpuid::EXT_FEATURE_INFO
        .as_ref()
        .is_some_and(cpuid::ExtendedFeatures::has_umip)
    {
        cr4_flags.insert(CR4Flags::UMIP);
    }

    if cpuid::EXT_FEATURE_INFO
        .as_ref()
        .is_some_and(cpuid::ExtendedFeatures::has_fsgsbase)
    {
        cr4_flags.insert(CR4Flags::FSGSBASE);
    }

    if cpuid::EXT_FEATURE_INFO
        .as_ref()
        .is_some_and(cpuid::ExtendedFeatures::has_smep)
    {
        cr4_flags.insert(CR4Flags::SMEP);
    }

    if cpuid::EXT_FEATURE_INFO
        .as_ref()
        .is_some_and(cpuid::ExtendedFeatures::has_smap)
    {
        cr4_flags.insert(CR4Flags::SMAP);
    }

    // Safety:  Initialize the CR4 register with all CPU & kernel supported features.
    unsafe {
        CR4::write(cr4_flags);
    }

    // Enable use of the `NO_EXECUTE` page attribute, if supported.
    if cpuid::EXT_FUNCTION_INFO
        .as_ref()
        .is_some_and(cpuid::ExtendedProcessorFeatureIdentifiers::has_execute_disable)
    {
        // Safety: The `NX` bit is not currently in use by any paging structures.
        unsafe {
            msr::IA32_EFER::set_nxe(true);
        }
    }

    // Safety: This function is only called once, prior to FS/GS base being in use.
    unsafe {
        crate::arch::x86_64::structures::gdt::load();
    }

    crate::arch::x86_64::structures::idt::load();

    // Setup system call interface.
    // // Safety: Parameters are set according to the IA-32 SDM, and so should have no undetermined side-effects.
    // unsafe {
    //     // Configure system call environment registers.
    //     msr::IA32_STAR::set_selectors(gdt::kernel_code_selector().0, gdt::kernel_data_selector().0);
    //     msr::IA32_LSTAR::set_syscall(syscall::_syscall_entry);
    //     // We don't want to keep any flags set within the syscall (especially the interrupt flag).
    //     msr::IA32_FMASK::set_rflags_mask(RFlags::all().bits());
    //     // Enable `syscall`/`sysret`.
    //     msr::IA32_EFER::set_sce(true);
    // }

    info!(
        "Vendor              {}",
        cpuid::VENDOR_INFO
            .as_ref()
            .map_or("UNKNOWN", raw_cpuid::VendorInfo::as_str)
    );
}

/// Gets the ID of the current core.
#[allow(clippy::map_unwrap_or)]
pub fn get_hwthread_id() -> u32 {
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
