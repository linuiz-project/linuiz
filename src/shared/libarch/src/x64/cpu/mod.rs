use alloc::boxed::Box;

mod syscall;

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

fn init_registers() {
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

    // SAFETY:  Initialize the CR4 register with all CPU & kernel supported features.
    unsafe { CR4::write(flags) };

    // Enable use of the `NO_EXECUTE` page attribute, if supported.
    if cpuid::EXT_FUNCTION_INFO.as_ref().map_or(false, cpuid::ExtendedProcessorFeatureIdentifiers::has_execute_disable)
    {
        trace!("Detected support for paging execution prevention.");
        // SAFETY:  Setting `IA32_EFER.NXE` in this context is safe because the bootloader does not use the `NX` bit. However, the kernel does, so
        //          disabling it after paging is in control of the kernel is unsupported.
        unsafe { crate::x64::registers::msr::IA32_EFER::set_nxe(true) };
    } else {
        warn!("PC does not support the NX bit; system security will be compromised (this warning is purely informational).");
    }
}

/// SAFETY: Caller must ensure this method is called only once per core.
unsafe fn init_tables() {
    use crate::x64::{
        instructions::tables,
        structures::{
            gdt::Descriptor,
            idt::{InterruptDescriptorTable, StackTableIndex},
        },
    };
    use x86_64::{structures::tss::TaskStateSegment, VirtAddr};

    trace!("Configuring local tables (IDT, GDT).");

    // Always initialize GDT prior to configuring IDT.
    crate::x64::structures::gdt::init();

    /* IDT init */
    {
        // Due to the fashion in which the `x86_64` crate initializes the IDT entries,
        // it must be ensured that the handlers are set only *after* the GDT has been
        // properly initialized and loaded—otherwise, the `CS` value for the IDT entries
        // is incorrect, and this causes very confusing GPFs.
        let mut idt = Box::new(InterruptDescriptorTable::new());
        crate::x64::structures::idt::set_exception_handlers(idt.as_mut());
        crate::x64::structures::idt::set_stub_handlers(idt.as_mut());
        idt.load_unsafe();

        Box::leak(idt);
    }

    /* TSS init */
    {
        trace!("Configuring new TSS and loading via temp GDT.");

        let tss_ptr = {
            use core::mem::MaybeUninit;
            use libcommon::memory::stack_aligned_allocator;

            let tss = Box::new(TaskStateSegment::new());

            let allocate_tss_stack = |pages: usize| {
                VirtAddr::from_ptr::<MaybeUninit<()>>(
                    Box::leak(Box::new_uninit_slice_in(pages * 0x1000, stack_aligned_allocator())).as_ptr(),
                )
            };

            // TODO guard pages for these stacks ?
            tss.privilege_stack_table[0] = allocate_tss_stack(5);
            tss.interrupt_stack_table[StackTableIndex::Debug as usize] = allocate_tss_stack(2);
            tss.interrupt_stack_table[StackTableIndex::NonMaskable as usize] = allocate_tss_stack(2);
            tss.interrupt_stack_table[StackTableIndex::DoubleFault as usize] = allocate_tss_stack(2);
            tss.interrupt_stack_table[StackTableIndex::MachineCheck as usize] = allocate_tss_stack(2);

            Box::leak(tss) as *mut _
        };

        trace!("Configuring TSS descriptor for temp GDT.");
        let tss_descriptor = {
            use bit_field::BitField;

            let tss_ptr_u64 = tss_ptr as u64;

            let mut low = x86_64::structures::gdt::DescriptorFlags::PRESENT.bits();
            // base
            low.set_bits(16..40, tss_ptr_u64.get_bits(0..24));
            low.set_bits(56..64, tss_ptr_u64.get_bits(24..32));
            // limit (the `-1` is needed since the bound is inclusive, not exclusive)
            low.set_bits(0..16, (core::mem::size_of::<TaskStateSegment>() - 1) as u64);
            // type (0b1001 = available 64-bit tss)
            low.set_bits(40..44, 0b1001);

            // high 32 bits of base
            let mut high = 0;
            high.set_bits(0..32, tss_ptr_u64.get_bits(32..64));

            Descriptor::SystemSegment(low, high)
        };

        trace!("Loading in temp GDT to `ltr` the TSS.");
        // Store current GDT pointer to restore later.
        let cur_gdt = tables::sgdt();
        // Create temporary kernel GDT to avoid a GPF on switching to it.
        let mut temp_gdt = x86_64::structures::gdt::GlobalDescriptorTable::new();
        temp_gdt.add_entry(Descriptor::kernel_code_segment());
        temp_gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = temp_gdt.add_entry(tss_descriptor);

        // Load temp GDT ...
        temp_gdt.load_unsafe();
        // ... load TSS from temporary GDT ...
        tables::load_tss(tss_selector);
        // ... and restore cached GDT.
        tables::lgdt(&cur_gdt);

        trace!("TSS loaded, and temporary GDT trashed.");
    }
}

fn init_syscalls() {
    // SAFETY: Parameters are set according to the IA-32 SDM, and so should have no undetermined side-effects.
    unsafe {
        use crate::x64::registers::{msr, RFlags};
        use crate::x64::structures::gdt;

        // Configure system call environment registers.
        msr::IA32_STAR::set_selectors(*gdt::KCODE_SELECTOR.get().unwrap(), *gdt::KDATA_SELECTOR.get().unwrap());
        msr::IA32_LSTAR::set_syscall(syscall::syscall_handler);
        // We don't want to keep any flags set within the syscall (especially the interrupt flag).
        msr::IA32_FMASK::set_rflags_mask(RFlags::all());
        // Enable `syscall`/`sysret`.
        msr::IA32_EFER::set_sce(true);
    }
}

/// SAFETY: This function expects to be called only once per CPU core.
pub unsafe fn init() {
    init_registers();
    init_tables();
    init_syscalls();
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
    (unsafe { *crate::interrupts::INTERRUPT_HANDLER.get() })(irq_number, &mut control_flow_context, &mut arch_context);

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
