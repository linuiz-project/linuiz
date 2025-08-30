use crate::arch::x86_64::{
    cpuid::{extended_feature_info, feature_info},
    devices::local_apic::LocalApic,
    registers::{ProcessorFlags, model_specific::IA32_TSC_AUX},
    structures::{gdt::GlobalDescriptorTable, idt::InterruptDescriptorTable},
};

pub mod cpuid;
pub mod devices;
pub mod instructions;
pub mod rand;
pub mod registers;
pub mod structures;

/// # Safety
///
/// This function has the potential to modify CPU state in such a way as to
/// disrupt software execution. It should be run only once per processor at the
/// very beginning of code execution.
pub unsafe fn configure_processor() {
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

    // Safety:
    // - Caller is required to ensure no CPU features are in use.
    // - All enabled features have been checked for support.
    unsafe {
        cr0::CR0::enable(cr0::Flags::NE);
        cr0::CR0::enable(cr0::Flags::AM);

        if let Some(feature_info) = feature_info() {
            if feature_info.has_pge() {
                cr4::CR4::enable(cr4::Flags::PGE);
            }

            if feature_info.has_de() {
                cr4::CR4::enable(cr4::Flags::DE);
            }

            if feature_info.has_fxsave_fxstor() {
                cr4::CR4::enable(cr4::Flags::OSFXSR);
                cr4::CR4::enable(cr4::Flags::OSXMMEXCPT);
            }

            if feature_info.has_mce() {
                cr4::CR4::enable(cr4::Flags::MCE);
            }

            if feature_info.has_pcid() {
                cr4::CR4::enable(cr4::Flags::PCIDE);
            }
        }

        if let Some(extended_feature_info) = extended_feature_info() {
            if extended_feature_info.has_umip() {
                cr4::CR4::enable(cr4::Flags::UMIP);
            }

            if extended_feature_info.has_fsgsbase() {
                cr4::CR4::enable(cr4::Flags::FSGSBASE);
            }

            if extended_feature_info.has_smep() {
                cr4::CR4::enable(cr4::Flags::SMEP);
            }

            if extended_feature_info.has_smap() {
                cr4::CR4::enable(cr4::Flags::SMAP);
            }
        }
    }

    // Safety: Only the alignment check bit is set.
    unsafe {
        ProcessorFlags::write(ProcessorFlags::read() | ProcessorFlags::ALIGNMENT_CHECK);
    }

    GlobalDescriptorTable::load_static();
    InterruptDescriptorTable::load_static();

    LocalApic::reset();

    if cpuid::extended_feature_info().is_some_and(|cpuid| cpuid.has_rdpid())
        || cpuid::extended_feature_identifiers().is_some_and(|cpuid| cpuid.has_rdtscp())
    {
        let processor_id = LocalApic::get_id();
        // Safety:
        // - Model-specific register is checked to be supported.
        // - Local APIC IDs are unqiue, and NUMA/multi-socket is not currently
        //   supported.
        unsafe {
            IA32_TSC_AUX::set(processor_id);
        }
    }
}

/// Gets the ID of the current processor.
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
pub fn get_processor_id() -> u32 {
    LocalApic::get_id()
}
