pub mod syscall;
pub mod cpuid {
    pub use raw_cpuid::*;
    use spin::Lazy;

    pub static CPUID: Lazy<CpuId> = Lazy::new(|| CpuId::new());
    pub static FEATURE_INFO: Lazy<FeatureInfo> = Lazy::new(|| CPUID.get_feature_info().expect("no CPUID.01H support"));
    pub static EXT_FEATURE_INFO: Lazy<Option<ExtendedFeatures>> = Lazy::new(|| CPUID.get_extended_feature_info());
    pub static EXT_FUNCTION_INFO: Lazy<Option<ExtendedProcessorFeatureIdentifiers>> =
        Lazy::new(|| CPUID.get_extended_processor_and_feature_identifiers());
    pub static VENDOR_INFO: Lazy<Option<VendorInfo>> = Lazy::new(|| CPUID.get_vendor_info());
}

use alloc::boxed::Box;

/// Reads [`crate::regisers::x86_64::msr::IA32_APIC_BASE`] to determine whether the current core
/// is the bootstrap processor.
#[inline(always)]
pub fn is_bsp() -> bool {
    crate::x64::registers::msr::IA32_APIC_BASE::get_is_bsp()
}

/// Gets the vendor of the CPU.
pub fn get_vendor() -> Option<&'static str> {
    cpuid::VENDOR_INFO.as_ref().map(|info| info.as_str())
}

/// Gets the ID of the current core.
pub fn get_id() -> u32 {
    use cpuid::{CPUID, FEATURE_INFO};

    CPUID
        // IA32 SDM instructs to enumerate this leaf first...
        .get_extended_topology_info_v2()
        // ... this leaf second ...
        .or_else(|| CPUID.get_extended_topology_info())
        .and_then(|mut iter| iter.next())
        .map(|info| info.x2apic_id())
        // ... and finally, this leaf as an absolute fallback.
        .unwrap_or_else(|| FEATURE_INFO.initial_local_apic_id() as u32)
}

pub fn init_registers() {
    trace!("Loading x86-specific control registers to known state.");

    // Set CR0 flags.
    use crate::x64::registers::control::{CR0Flags, CR0};
    // SAFETY: We set `CR0` once, and setting it again during kernel execution is not supported.
    unsafe { CR0::write(CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG) };

    // Set CR4 flags.
    use crate::x64::registers::control::{CR4Flags, CR4};
    use cpuid::{EXT_FEATURE_INFO, FEATURE_INFO};

    let mut flags = CR4Flags::PAE | CR4Flags::PGE | CR4Flags::OSXMMEXCPT;

    if FEATURE_INFO.has_de() {
        trace!("Detected support for debugging extensions.");
        flags.insert(CR4Flags::DE);
    }

    if FEATURE_INFO.has_fxsave_fxstor() {
        trace!("Detected support for `fxsave` and `fxstor` instructions.");
        flags.insert(CR4Flags::OSFXSR);
    }

    if FEATURE_INFO.has_mce() {
        trace!("Detected support for machine check exceptions.");
    }

    if FEATURE_INFO.has_pcid() {
        trace!("Detected support for process context IDs.");
        flags.insert(CR4Flags::PCIDE);
    }

    if EXT_FEATURE_INFO.as_ref().map_or(false, cpuid::ExtendedFeatures::has_umip) {
        trace!("Detected support for usermode instruction prevention.");
        flags.insert(CR4Flags::UMIP);
    }

    if EXT_FEATURE_INFO.as_ref().map_or(false, cpuid::ExtendedFeatures::has_fsgsbase) {
        trace!("Detected support for CPL3 FS/GS base usage.");
        flags.insert(CR4Flags::FSGSBASE);
    }

    if EXT_FEATURE_INFO.as_ref().map_or(false, cpuid::ExtendedFeatures::has_smep) {
        trace!("Detected support for supervisor mode execution prevention.");
        flags.insert(CR4Flags::SMEP);
    }

    if EXT_FEATURE_INFO.as_ref().map_or(false, cpuid::ExtendedFeatures::has_smap) {
        trace!("Detected support for supervisor mode access prevention.");
        // TODO flags.insert(CR4Flags::SMAP);
    }

    // SAFETY: Initialize the CR4 register with all CPU & kernel supported features.
    unsafe { CR4::write(flags) };

    // Enable use of the `NO_EXECUTE` page attribute, if supported.
    if cpuid::EXT_FUNCTION_INFO.as_ref().map_or(false, cpuid::ExtendedProcessorFeatureIdentifiers::has_execute_disable)
    {
        trace!("Detected support for paging execution prevention.");
        // SAFETY: Setting `IA32_EFER.NXE` in this context is safe because the bootloader does not use the `NX` bit. However, the kernel does, so
        //         disabling it after paging is in control of the kernel is unsupported.
        unsafe { crate::x64::registers::msr::IA32_EFER::set_nxe(true) };
    } else {
        warn!("PC does not support the NX bit; system security will be compromised (this warning is purely informational).");
    }
}



pub fn init_syscalls() {
    // SAFETY: Parameters are set according to the IA-32 SDM, and so should have no undetermined side-effects.
    unsafe {
        use crate::x64::registers::{msr, RFlags};
        use crate::x64::structures::gdt;

        // Configure system call environment registers.
        msr::IA32_STAR::set_selectors(*gdt::KCODE_SELECTOR.get().unwrap(), *gdt::KDATA_SELECTOR.get().unwrap());
        msr::IA32_LSTAR::set_syscall(syscall::_syscall_entry);
        // We don't want to keep any flags set within the syscall (especially the interrupt flag).
        msr::IA32_FMASK::set_rflags_mask(RFlags::all());
        // Enable `syscall`/`sysret`.
        msr::IA32_EFER::set_sce(true);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpecialContext {
    pub cs: u64,
    pub ss: u64,
    pub flags: crate::x64::registers::RFlags,
}

impl SpecialContext {
    pub fn with_kernel_segments(flags: crate::x64::registers::RFlags) -> Self {
        Self {
            cs: crate::x64::structures::gdt::KCODE_SELECTOR.get().unwrap().0 as u64,
            ss: crate::x64::structures::gdt::KDATA_SELECTOR.get().unwrap().0 as u64,
            flags,
        }
    }

    pub fn flags_with_user_segments(flags: crate::x64::registers::RFlags) -> Self {
        Self {
            cs: crate::x64::structures::gdt::UCODE_SELECTOR.get().unwrap().0 as u64,
            ss: crate::x64::structures::gdt::UDATA_SELECTOR.get().unwrap().0 as u64,
            flags,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GeneralContext {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

impl GeneralContext {
    pub const fn empty() -> Self {
        Self {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        }
    }
}

/// Hand the interrupt context off to the common interrupt handler.
pub(in crate::x64) extern "sysv64" fn irq_handoff(
    irq_number: u64,
    stack_frame: &mut crate::x64::structures::idt::InterruptStackFrame,
    general_context: &mut GeneralContext,
) {
    let mut control_flow_context = crate::interrupts::ControlFlowContext {
        ip: stack_frame.instruction_pointer.as_u64(),
        sp: stack_frame.stack_pointer.as_u64(),
    };

    let mut arch_context = (
        *general_context,
        SpecialContext {
            cs: stack_frame.code_segment,
            ss: stack_frame.stack_segment,
            flags: crate::x64::registers::RFlags::from_bits_truncate(stack_frame.cpu_flags),
        },
    );

    // SAFETY: function pointer is guaranteed by the `set_interrupt_handler()` function to be valid.
    unsafe { crate::interrupts::IRQ_HANDLER(irq_number, &mut control_flow_context, &mut arch_context) };

    // SAFETY: The stack frame *has* to be modified to switch contexts within this interrupt.
    unsafe {
        use x86_64::VirtAddr;

        stack_frame.as_mut().write(crate::x64::structures::idt::InterruptStackFrameValue {
            instruction_pointer: VirtAddr::new(control_flow_context.ip),
            stack_pointer: VirtAddr::new(control_flow_context.sp),
            code_segment: arch_context.1.cs,
            stack_segment: arch_context.1.ss,
            cpu_flags: arch_context.1.flags.bits(),
        });

        *general_context = arch_context.0;
    };
}
