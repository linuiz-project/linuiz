use raw_cpuid::{ExtendedFeatures, ExtendedProcessorFeatureIdentifiers, FeatureInfo};

use crate::arch::x86_64::{
    cpuid::{extended_feature_identifiers, extended_feature_info, feature_info},
    devices::x2apic::x2Apic,
    structures::{gdt::GlobalDescriptorTable, idt::InterruptDescriptorTable},
};

pub mod cpuid;
pub mod devices;
pub mod instructions;
pub mod registers;
pub mod structures;

/// # Safety
///
/// This function has the potential to modify CPU state in such a way as to disrupt
/// software execution. It should be run only once per hardware thread at the very
/// beginning of code execution.
pub unsafe fn configure_hwthread() {
    use registers::{
        control::{CR0, CR0Flags, CR4, CR4Flags},
        model_specific::IA32_EFER,
    };

    trace!("Configuring `CR0`...");

    // Safety: This is the first and only time `CR0` will be set.
    unsafe {
        CR0::write(
            CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG,
        );
    }

    trace!("Configuring `CR4`...");

    let mut cr4_flags = CR4Flags::PAE | CR4Flags::PGE | CR4Flags::OSXMMEXCPT;

    if feature_info().is_some_and(FeatureInfo::has_de) {
        cr4_flags.insert(CR4Flags::DE);
    }

    if feature_info().is_some_and(FeatureInfo::has_fxsave_fxstor) {
        cr4_flags.insert(CR4Flags::OSFXSR);
    }

    if feature_info().is_some_and(FeatureInfo::has_mce) {
        cr4_flags.insert(CR4Flags::MCE);
    }

    if feature_info().is_some_and(FeatureInfo::has_pcid) {
        cr4_flags.insert(CR4Flags::PCIDE);
    }

    if extended_feature_info().is_some_and(ExtendedFeatures::has_umip) {
        cr4_flags.insert(CR4Flags::UMIP);
    }

    if extended_feature_info().is_some_and(ExtendedFeatures::has_fsgsbase) {
        cr4_flags.insert(CR4Flags::FSGSBASE);
    }

    if extended_feature_info().is_some_and(ExtendedFeatures::has_smep) {
        cr4_flags.insert(CR4Flags::SMEP);
    }

    if extended_feature_info().is_some_and(ExtendedFeatures::has_smap) {
        cr4_flags.insert(CR4Flags::SMAP);
    }

    // Safety:  Initialize the CR4 register with all CPU & kernel supported features.
    unsafe {
        CR4::write(cr4_flags);
    }

    trace!("Configuring `IA32_EFER.NXE`...");

    // Enable use of the `NO_EXECUTE` page attribute, if supported.
    if extended_feature_identifiers()
        .is_some_and(ExtendedProcessorFeatureIdentifiers::has_execute_disable)
    {
        trace!("Set `IA32_EFER.NXE`.");
        IA32_EFER::set_no_execute_enable(true);
    }

    GlobalDescriptorTable::init();
    GlobalDescriptorTable::load_static();

    InterruptDescriptorTable::init();
    InterruptDescriptorTable::load_static();

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
}

/// Gets the ID of the current core.
///
/// # Remarks
///
/// Currently, this effectively just reads the 32-bit ID provided by the x2APIC
/// controller. In the future, obviously there is interest in supporting identification
/// of more diverse hardware layouts than just assuming a flat CPU model that disregards
/// even hyper-threading (which is all but ubiquitous in 2025, at time of writing). So,
/// it can be expected that in the future, this function will change significantly.
#[allow(clippy::map_unwrap_or)]
pub fn get_hwthread_id() -> u32 {
    x2Apic::get_id()
}
