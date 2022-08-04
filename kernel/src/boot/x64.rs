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
    crate::cpu_setup(true)
}

/// Entrypoint for AP processors.
#[inline(never)]
unsafe extern "C" fn _smp_entry() -> ! {
    // Wait to ensure the machine is the correct state to execute cpu setup.
    while !SMP_MEMORY_READY.load(core::sync::atomic::Ordering::Relaxed) {}

    crate::cpu_setup(false)
}
