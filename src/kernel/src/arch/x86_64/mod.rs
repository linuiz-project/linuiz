use crate::arch::x86_64::{
    cpuid::{extended_feature_info, feature_info},
    devices::local_apic::LAPIC,
    registers::ProcessorFlags,
    structures::{gdt::GlobalDescriptorTable, idt::InterruptDescriptorTable},
};
use raw_cpuid::{ExtendedFeatures, FeatureInfo};

pub mod cpuid;
pub mod devices;
pub mod instructions;
pub mod rand;
pub mod registers;
pub mod structures;

/// # Safety
///
/// This function has the potential to modify CPU state in such a way as to
/// disrupt software execution. It should be run only once per hardware thread
/// at the very beginning of code execution.
pub unsafe fn configure_hwthread() {
    use registers::{
        control::{cr0, cr4},
        model_specific::IA32_EFER,
    };

    // Double-check flags set by bootloader.
    debug_assert!(cr0::CR0::read().contains(cr0::Flags::PG));
    debug_assert!(cr0::CR0::read().contains(cr0::Flags::PE));
    debug_assert!(cr0::CR0::read().contains(cr0::Flags::WP));
    debug_assert!(cr4::CR4::read().contains(cr4::Flags::PAE));
    debug_assert!(IA32_EFER::get_long_mode_active());
    debug_assert!(IA32_EFER::get_no_execute_enable());

    // Safety: No invalid features are enabled.
    unsafe {
        cr0::CR0::enable(cr0::Flags::NE | cr0::Flags::AM);
    }

    let mut flags = cr4::Flags::empty();

    if feature_info().is_some_and(FeatureInfo::has_pge) {
        flags.insert(cr4::Flags::PGE);
    }

    if feature_info().is_some_and(FeatureInfo::has_de) {
        flags.insert(cr4::Flags::DE);
    }

    if feature_info().is_some_and(FeatureInfo::has_fxsave_fxstor) {
        flags.insert(cr4::Flags::OSFXSR);
        flags.insert(cr4::Flags::OSXMMEXCPT);
    }

    if feature_info().is_some_and(FeatureInfo::has_mce) {
        flags.insert(cr4::Flags::MCE);
    }

    if feature_info().is_some_and(FeatureInfo::has_pcid) {
        flags.insert(cr4::Flags::PCIDE);
    }

    if extended_feature_info().is_some_and(ExtendedFeatures::has_umip) {
        flags.insert(cr4::Flags::UMIP);
    }

    if extended_feature_info().is_some_and(ExtendedFeatures::has_fsgsbase) {
        flags.insert(cr4::Flags::FSGSBASE);
    }

    if extended_feature_info().is_some_and(ExtendedFeatures::has_smep) {
        flags.insert(cr4::Flags::SMEP);
    }

    if extended_feature_info().is_some_and(ExtendedFeatures::has_smap) {
        flags.insert(cr4::Flags::SMAP);
    }

    // Safety:
    // - Caller is required to ensure no CPU features are in use.
    // - All enabled features have been checked for support.
    unsafe {
        cr4::CR4::enable(flags);
    }

    // Safety: Only the alignment check bit is set.
    unsafe {
        ProcessorFlags::write(ProcessorFlags::read() | ProcessorFlags::ALIGNMENT_CHECK);
    }

    GlobalDescriptorTable::init();
    GlobalDescriptorTable::load_static();

    InterruptDescriptorTable::init();
    InterruptDescriptorTable::load_static();

    // Setup system call interface.
    // // Safety: Parameters are set according to the IA-32 SDM, and so should
    // have no undetermined side-effects. unsafe {
    //     // Configure system call environment registers.
    //     msr::IA32_STAR::set_selectors(gdt::kernel_code_selector().0,
    // gdt::kernel_data_selector().0);
    //     msr::IA32_LSTAR::set_syscall(syscall::_syscall_entry);
    //     // We don't want to keep any flags set within the syscall (especially
    // the interrupt flag).
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
/// controller. In the future, obviously there is interest in supporting
/// identification of more diverse hardware layouts than just assuming a flat
/// CPU model that disregards even hyper-threading (which is all but ubiquitous
/// in 2025, at time of writing). So, it can be expected that in the future,
/// this function will return a more dynamic identification structure.
#[allow(clippy::map_unwrap_or)]
pub fn get_hwthread_id() -> u32 {
    LAPIC.get_id()
}
