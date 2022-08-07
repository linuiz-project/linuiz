const LIMINE_REV: u64 = 0;
static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(LIMINE_REV);
static LIMINE_RSDP: limine::LimineRsdpRequest = limine::LimineRsdpRequest::new(LIMINE_REV);
static LIMINE_INF: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(LIMINE_REV);
static LIMINE_MMAP: limine::LimineMmapRequest = limine::LimineMmapRequest::new(LIMINE_REV);
static LIMINE_HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(LIMINE_REV);

const DEV_UNMAP_LOWER_HALF_IDMAP: bool = false;
static mut CON_OUT: crate::drivers::stdout::Serial = crate::drivers::stdout::Serial::new(crate::drivers::stdout::COM1);
static SMP_MEMORY_READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

#[no_mangle]
unsafe extern "sysv64" fn _entry() -> ! {
    CON_OUT.init(crate::drivers::stdout::SerialSpeed::S115200);
    match crate::drivers::stdout::set_stdout(&mut CON_OUT, log::LevelFilter::Debug) {
        Ok(()) => info!("Successfully loaded into kernel."),
        Err(_) => libkernel::instructions::interrupts::wait_indefinite(),
    }

    /* log boot info */
    {
        let boot_info = LIMINE_INF.get_response().get().expect("bootloader provided no info");
        info!(
            "Bootloader Info     {} v{} (rev {})",
            core::ffi::CStr::from_ptr(boot_info.name.as_ptr().unwrap() as *const _).to_str().unwrap(),
            core::ffi::CStr::from_ptr(boot_info.version.as_ptr().unwrap() as *const _).to_str().unwrap(),
            boot_info.revision,
        );

        if let Some(vendor_str) = libkernel::cpu::get_vendor() {
            info!("Vendor              {}", vendor_str);
        } else {
            info!("Vendor              None");
        }
    }

    /* prepare APs for startup */
    // TODO add a kernel parameter for SMP
    {
        let smp_response =
            LIMINE_SMP.get_response().as_mut_ptr().expect("received no SMP response from bootloader").as_mut().unwrap();

        if let Some(cpus) = smp_response.cpus() {
            debug!("Detected {} APs.", cpus.len() - 1);

            for cpu_info in cpus {
                // Ensure we don't try to 'start' the BSP.
                if cpu_info.lapic_id != smp_response.bsp_lapic_id {
                    debug!("Starting processor: PID{}/LID{}", cpu_info.processor_id, cpu_info.lapic_id);
                    cpu_info.goto_address = _smp_entry as u64;
                }
            }
        }
    }

    /* load RSDP pointer */
    {
        // TODO Possibly move ACPI structure instances out of libkernel?
        // Set RSDP pointer, so ACPI can be used.
        libkernel::acpi::set_rsdp_ptr(
            LIMINE_RSDP
                .get_response()
                .get()
                .expect("bootloader provided to RSDP pointer (no ACPI support)")
                .address
                .as_ptr()
                .unwrap() as *const _,
        );
    }

    /* init memory */
    {
        use libkernel::memory::Page;

        trace!("Initializing memory managers.");

        let memory_map = LIMINE_MMAP
            .get_response()
            .get()
            .and_then(|resp| resp.mmap())
            .expect("no memory map has been provided by bootloader");

        // Frame manager is always initialized first, so virtual structures may allocate frames.
        crate::memory::init_kernel_frame_manager(memory_map);

        // Next, we create the kernel page manager, utilizing the bootloader's higher-half direct
        // mapping for virtual offset mapping.
        let hhdm_addr = libkernel::Address::<libkernel::Virtual>::new(
            LIMINE_HHDM.get_response().get().expect("bootloader did not provide a higher half direct mapping").offset
                as usize,
        );
        trace!("Higher half identity mapping base: {:?}", hhdm_addr);
        crate::memory::init_kernel_page_manager(hhdm_addr);

        let frame_manager = crate::memory::get_kernel_frame_manager().unwrap();
        // The frame manager's allocation table is allocated with identity mapping assumed,
        // so before we unmap the lower half virtual memory mapping (for kernel heap), we
        // must ensure the frame manager uses the HHDM base.
        frame_manager.slide_table_base(hhdm_addr.as_usize());

        if DEV_UNMAP_LOWER_HALF_IDMAP {
            let page_manager = crate::memory::get_kernel_page_manager().unwrap();
            trace!("Unmapping lower half identity mappings.");
            for entry in memory_map.iter() {
                for page in (entry.base..(entry.base + entry.len))
                    .step_by(0x1000)
                    .map(|base| Page::from_index((base / 0x1000) as usize))
                {
                    // TODO maybe sometimes this fails? It did before, but isn't now. Could be because of an update to Limine.
                    page_manager.unmap(&page, libkernel::memory::FrameOwnership::None, frame_manager).unwrap();
                }
            }
        }

        // The global kernel allocator must be set AFTER the upper half
        // identity mappings are purged, so that the allocation table
        // (which will reside in the lower half) isn't unmapped.
        trace!("Assigning libkernel global allocator.");
        libkernel::memory::global_alloc::set(&*crate::KMALLOC);
    }

    debug!("Finished initial kernel setup.");
    SMP_MEMORY_READY.store(true, core::sync::atomic::Ordering::Relaxed);
    arch_setup(true)
}

/// Entrypoint for AP processors.
#[inline(never)]
unsafe extern "C" fn _smp_entry() -> ! {
    // Wait to ensure the machine is the correct state to execute cpu setup.
    while !SMP_MEMORY_READY.load(core::sync::atomic::Ordering::Relaxed) {}

    arch_setup(false)
}

/// SAFETY: This function invariantly assumes it will only be called once.
unsafe fn arch_setup(is_bsp: bool) -> ! {
    /* load registers */
    {
        // Set CR0 flags.
        use libkernel::registers::x64::control::{CR0Flags, CR0};
        CR0::write(CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG);

        // Set CR4 flags.
        use libkernel::{
            cpu::x64::{EXT_FEATURE_INFO, FEATURE_INFO},
            registers::x64::control::{CR4Flags, CR4},
        };

        let mut flags = CR4Flags::PAE | CR4Flags::PGE | CR4Flags::OSXMMEXCPT;

        if FEATURE_INFO.as_ref().map(|info| info.has_de()).unwrap_or(false) {
            trace!("Detected support for debugging extensions.");
            flags.insert(CR4Flags::DE);
        }

        if FEATURE_INFO.as_ref().map(|info| info.has_fxsave_fxstor()).unwrap_or(false) {
            trace!("Detected support for `fxsave` and `fxstor` instructions.");
            flags.insert(CR4Flags::OSFXSR);
        }

        if FEATURE_INFO.as_ref().map(|info| info.has_mce()).unwrap_or(false) {
            trace!("Detected support for machine check exceptions.")
        }

        if FEATURE_INFO.as_ref().map(|info| info.has_pcid()).unwrap_or(false) {
            trace!("Detected support for process context IDs.");
            flags.insert(CR4Flags::PCIDE);
        }

        if EXT_FEATURE_INFO.as_ref().map(|info| info.has_umip()).unwrap_or(false) {
            trace!("Detected support for usermode instruction prevention.");
            flags.insert(CR4Flags::UMIP);
        }

        if EXT_FEATURE_INFO.as_ref().map(|info| info.has_fsgsbase()).unwrap_or(false) {
            trace!("Detected support for CPL3 FS/GS base usage.");
            flags.insert(CR4Flags::FSGSBASE);
        }

        if EXT_FEATURE_INFO.as_ref().map(|info| info.has_smep()).unwrap_or(false) {
            trace!("Detected support for supervisor mode execution prevention.");
            flags.insert(CR4Flags::SMEP);
        }

        if EXT_FEATURE_INFO.as_ref().map(|info| info.has_smap()).unwrap_or(false) {
            trace!("Detected support for supervisor mode access prevention.");
            flags.insert(CR4Flags::SMAP);
        }

        CR4::write(flags);

        // Enable use of the `NO_EXECUTE` page attribute, if supported.
        if libkernel::cpu::x64::EXT_FUNCTION_INFO
            .as_ref()
            .map(|func_info| func_info.has_execute_disable())
            .unwrap_or(false)
        {
            libkernel::registers::x64::msr::IA32_EFER::set_nxe(true);
        } else {
            warn!("PC does not support the NX bit; system security will be compromised (this warning is purely informational).")
        }
    }

    /* load tables */
    {
        trace!("Configuring local tables (IDT, GDT).");

        // Always initialize GDT prior to configuring IDT.
        crate::tables::gdt::init();

        if is_bsp {
            // Due to the fashion in which the `x86_64` crate initializes the IDT entries,
            // it must be ensured that the handlers are set only *after* the GDT has been
            // properly initialized and loaded—otherwise, the `CS` value for the IDT entries
            // is incorrect, and this causes very confusing GPFs.
            crate::interrupts::init_idt();

            fn apit_empty(
                _: &mut x86_64::structures::idt::InterruptStackFrame,
                _: &mut crate::scheduling::ThreadRegisters,
            ) {
                libkernel::structures::apic::end_of_interrupt();
            }

            crate::interrupts::set_handler_fn(crate::interrupts::Vector::LINT0_VECTOR, apit_empty);
            crate::interrupts::set_handler_fn(crate::interrupts::Vector::LINT1_VECTOR, apit_empty);
            crate::interrupts::set_handler_fn(crate::interrupts::Vector::Syscall, crate::interrupts::syscall::handler);
        }

        crate::interrupts::load_idt();

        /* load tss */
        use alloc::boxed::Box;
        use libkernel::{
            interrupts::StackTableIndex,
            memory::{page_aligned_allocator, PageAlignedBox},
        };
        use x86_64::{
            instructions::tables,
            structures::{
                gdt::{Descriptor, GlobalDescriptorTable},
                tss::TaskStateSegment,
            },
            VirtAddr,
        };

        const PRIVILEGE_STACK_SIZE: usize = 0x5000;
        const EXCEPTION_STACK_SIZE: usize = 0x2000;

        let privilege_stack_ptr =
            Box::leak(PageAlignedBox::<[u8]>::new_uninit_slice_in(PRIVILEGE_STACK_SIZE, page_aligned_allocator()))
                .as_mut_ptr()
                .add(PRIVILEGE_STACK_SIZE);
        let db_stack_ptr =
            Box::leak(PageAlignedBox::<[u8]>::new_uninit_slice_in(EXCEPTION_STACK_SIZE, page_aligned_allocator()))
                .as_mut_ptr()
                .add(EXCEPTION_STACK_SIZE);
        let nmi_stack_ptr =
            Box::leak(PageAlignedBox::<[u8]>::new_uninit_slice_in(EXCEPTION_STACK_SIZE, page_aligned_allocator()))
                .as_mut_ptr()
                .add(EXCEPTION_STACK_SIZE);
        let df_stack_ptr =
            Box::leak(PageAlignedBox::<[u8]>::new_uninit_slice_in(EXCEPTION_STACK_SIZE, page_aligned_allocator()))
                .as_mut_ptr()
                .add(EXCEPTION_STACK_SIZE);
        let mc_stack_ptr =
            Box::leak(PageAlignedBox::<[u8]>::new_uninit_slice_in(EXCEPTION_STACK_SIZE, page_aligned_allocator()))
                .as_mut_ptr()
                .add(EXCEPTION_STACK_SIZE);

        trace!("Configuring new TSS and loading via temp GDT.");

        let tss_ptr = Box::leak({
            let mut tss = Box::new(x86_64::structures::tss::TaskStateSegment::new());

            unsafe {
                tss.privilege_stack_table[0] = VirtAddr::from_ptr(privilege_stack_ptr);
                tss.interrupt_stack_table[StackTableIndex::Debug as usize] = VirtAddr::from_ptr(db_stack_ptr);
                tss.interrupt_stack_table[StackTableIndex::NonMaskable as usize] = VirtAddr::from_ptr(nmi_stack_ptr);
                tss.interrupt_stack_table[StackTableIndex::DoubleFault as usize] = VirtAddr::from_ptr(df_stack_ptr);
                tss.interrupt_stack_table[StackTableIndex::MachineCheck as usize] = VirtAddr::from_ptr(mc_stack_ptr);
            }

            tss
        }) as *mut _;

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
        let mut temp_gdt = GlobalDescriptorTable::new();
        temp_gdt.add_entry(Descriptor::kernel_code_segment());
        temp_gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = temp_gdt.add_entry(tss_descriptor);

        // Load temp GDT ...
        temp_gdt.load_unsafe();
        // ... load TSS from temporary GDT ...
        tables::load_tss(tss_selector);
        // ... and restore cached GDT.
        tables::lgdt(&cur_gdt);

        trace!("TSS loaded.");
    }

    trace!("Arch-specific local setup complete.");
    crate::cpu_setup(is_bsp)
}
