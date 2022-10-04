use libcommon::{Address, Frame, Page, Virtual};

pub struct Parameters {
    pub smp: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Self { smp: true }
    }
}

static PARAMETERS: spin::Lazy<Parameters> = spin::Lazy::new(|| {
    let mut parameters = Parameters::default();

    if let Some(cmdline) = get_kernel_file()
        .and_then(|kernel_file| kernel_file.cmdline.to_str())
        .and_then(|cmdline_cstr| cmdline_cstr.to_str().ok())
        .map(|cmdline| cmdline.split(' '))
    {
        cmdline.for_each(|parameter| match parameter.split_once(':') {
            Some(("smp", "on")) => parameters.smp = true,
            Some(("smp", "off")) => parameters.smp = false,

            _ => warn!("Unhandled cmdline parameter: {:?}", parameter),
        })
    }

    parameters
});

pub static MEMORY_READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Brings up the UART serial interface, if it exists.
pub fn serial() {
    static UART: spin::Lazy<crate::memory::io::Serial> = spin::Lazy::new(|| {
        // SAFETY: Function is called only once, when the `Lazy` is initialized.
        unsafe { crate::memory::io::Serial::init() }
    });

    log::set_max_level(log::LevelFilter::Trace);
    log::set_logger(&*UART).unwrap();
}

pub fn boot_info() {
    static LIMINE_INFO: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(crate::LIMINE_REV);

    if let Some(boot_info) = LIMINE_INFO.get_response().get() {
        use core::ffi::CStr;

        info!(
            "Bootloader Info     {} v{} (rev {})",
            boot_info
                .name
                .as_ptr()
                .map(|ptr| unsafe { CStr::from_ptr(ptr) })
                .and_then(|cstr| cstr.to_str().ok())
                .unwrap_or("Unknown"),
            boot_info
                .version
                .as_ptr()
                .map(|ptr| unsafe { CStr::from_ptr(ptr) })
                .and_then(|cstr| cstr.to_str().ok())
                .unwrap_or("0"),
            boot_info.revision
        );
    }

    #[cfg(target_arch = "x86_64")]
    if let Some(vendor_str) = libarch::x64::cpu::get_vendor() {
        info!("Vendor              {vendor_str}");
    } else {
        info!("Vendor              None");
    }
}

fn get_kernel_file() -> Option<&'static limine::LimineFile> {
    static LIMINE_KERNEL_FILE: limine::LimineKernelFileRequest = limine::LimineKernelFileRequest::new(0);

    LIMINE_KERNEL_FILE.get_response().get().and_then(|response| response.kernel_file.get())
}

pub fn smp() {
    static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(crate::LIMINE_REV).flags(0b1);

    if let Some(smp_response) = LIMINE_SMP.get_response().get_mut() {
        debug!("Detected {} additional cores.", smp_response.cpu_count - 1);
        for cpu_info in smp_response.cpus().iter().filter(|info| info.lapic_id != smp_response.bsp_lapic_id) {
            trace!("Starting processor: ID P{}/L{}", cpu_info.processor_id, cpu_info.lapic_id);

            cpu_info.goto_address = if PARAMETERS.smp {
                extern "C" fn _smp_entry(info: *const limine::LimineSmpInfo) -> ! {
                    // SAFETY: Bootloader ensures this pointer is unique to this thread.
                    let info = unsafe { &*info };

                    // Wait to ensure the machine is the correct state to execute cpu setup.
                    while !MEMORY_READY.load(core::sync::atomic::Ordering::Relaxed) {
                        core::hint::spin_loop();
                    }

                    // SAFETY: Function is called only once per core.
                    #[cfg(target_arch = "x86_64")]
                    unsafe {
                        libarch::x64::cpu::init()
                    };

                    // SAFETY: All currently referenced memory should also be mapped in the kernel page tables.
                    unsafe { crate::memory::get_kernel_mapper().commit_vmem_register().unwrap() };

                    trace!("Finished SMP entry for core.");

                    // SAFETY: Function is called only once for this core.
                    unsafe { crate::kernel_thread_setup(info.lapic_id) }
                }

                _smp_entry
            } else {
                extern "C" fn _idle_forever(_: *const limine::LimineSmpInfo) -> ! {
                    // SAFETY: Murder isn't legal. Is this?
                    unsafe { libarch::interrupts::halt_and_catch_fire() }
                }

                _idle_forever
            };
        }
    } else {
        debug!("Bootloader has not provided any SMP information.");
    }
}

/// SAFETY: This function expects to be called only once.
pub unsafe fn memory() {
    crate::memory::init_kernel_allocator(LIMINE_MMAP.get_response().get().unwrap().memmap());

    let to_mapper = crate::memory::get_kernel_mapper();

    /* memory init */
    {
        use crate::memory::PageAttributes;
        use crate::memory::VirtualMapper;
        use libcommon::{LinkerSymbol, PageAlign};

        extern "C" {
            static __text_start: LinkerSymbol;
            static __text_end: LinkerSymbol;

            static __rodata_start: LinkerSymbol;
            static __rodata_end: LinkerSymbol;

            static __bss_start: LinkerSymbol;
            static __bss_end: LinkerSymbol;

            static __data_start: LinkerSymbol;
            static __data_end: LinkerSymbol;
        }

        debug!("Initializing kernel page manager...");

        let hhdm_address = crate::memory::get_hhdm_address();
        let from_mapper = VirtualMapper::from_current(hhdm_address);

        {
            fn map_range_from(
                from_mapper: &VirtualMapper,
                to_mapper: &VirtualMapper,
                range: core::ops::Range<u64>,
                attributes: PageAttributes,
            ) {
                // SAFETY: Linker should have correctly set this value.
                let page_align = unsafe {
                    extern "C" {
                        static __SECTION_ALIGN: LinkerSymbol;
                    }

                    PageAlign::from_u64(__SECTION_ALIGN.as_u64()).unwrap()
                };

                for address in range.step_by(page_align.as_usize()).map(Address::<Virtual>::new_truncate) {
                    to_mapper
                        .map(
                            Address::<Page>::new_truncate(address, page_align),
                            // Properly handle the bootloader possibly not using huge page mappings (but still physically contiguous).
                            Address::<Page>::new(address, None)
                                .and_then(|page_address| from_mapper.get_mapped_to(page_address))
                                .unwrap(),
                            false,
                            attributes,
                        )
                        .unwrap();
                }
            }

            map_range_from(
                &from_mapper,
                &to_mapper,
                __text_start.as_u64()..__text_end.as_u64(),
                PageAttributes::RX | PageAttributes::GLOBAL,
            );
            map_range_from(
                &from_mapper,
                &to_mapper,
                __rodata_start.as_u64()..__rodata_end.as_u64(),
                PageAttributes::RO | PageAttributes::GLOBAL,
            );
            map_range_from(
                &from_mapper,
                &to_mapper,
                __bss_start.as_u64()..__bss_end.as_u64(),
                PageAttributes::RW | PageAttributes::GLOBAL,
            );
            map_range_from(
                &from_mapper,
                &to_mapper,
                __data_start.as_u64()..__data_end.as_u64(),
                PageAttributes::RW | PageAttributes::GLOBAL,
            );
        }

        for entry in LIMINE_MMAP.get_response().get().unwrap().memmap() {
            let page_attributes = {
                use limine::LimineMemoryMapEntryType;
                match entry.typ {
                        LimineMemoryMapEntryType::Usable
                        | LimineMemoryMapEntryType::AcpiNvs
                        | LimineMemoryMapEntryType::AcpiReclaimable
                        | LimineMemoryMapEntryType::BootloaderReclaimable
                        // TODO handle the PATs or something to make this WC
                        | LimineMemoryMapEntryType::Framebuffer => PageAttributes::RW,

                        LimineMemoryMapEntryType::Reserved | LimineMemoryMapEntryType::KernelAndModules => {
                            PageAttributes::RO
                        }

                        LimineMemoryMapEntryType::BadMemory => continue,
                    }
            };

            for phys_base in (entry.base..(entry.base + entry.len)).step_by(0x1000) {
                // TODO use huge pages here if possible
                to_mapper
                    .map(
                        Address::<Page>::new_truncate(
                            Address::<Virtual>::new_truncate(hhdm_address.as_u64() + phys_base),
                            PageAlign::Align4KiB,
                        ),
                        Address::<Frame>::new_truncate(phys_base),
                        false,
                        page_attributes,
                    )
                    .unwrap();
            }
        }
    }
}
