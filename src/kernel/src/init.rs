use libcommon::{Address, Frame, Page, Virtual};

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub smp: bool,
    pub symbolinfo: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Self { smp: true, symbolinfo: false }
    }
}

static PARAMETERS: spin::Once<Parameters> = spin::Once::new();

pub fn get_parameters() -> Parameters {
    PARAMETERS.get().map(Parameters::clone).unwrap_or_default()
}

fn get_kernel_file() -> Option<&'static limine::LimineFile> {
    static LIMINE_KERNEL_FILE: limine::LimineKernelFileRequest = limine::LimineKernelFileRequest::new(0);

    LIMINE_KERNEL_FILE.get_response().get().and_then(|response| response.kernel_file.get())
}

fn get_memory_map() -> Option<&'static [limine::NonNullPtr<limine::LimineMemmapEntry>]> {
    static LIMINE_MMAP: limine::LimineMemmapRequest = limine::LimineMemmapRequest::new(crate::LIMINE_REV);

    LIMINE_MMAP.get_response().get().map(limine::LimineMemmapResponse::memmap)
}

pub fn init() {
    use core::sync::atomic::{AtomicBool, Ordering};
    for init_stage in INIT_STAGES {
        init_stage();
    }
    static HAS_INIT: AtomicBool = AtomicBool::new(false);

    match HAS_INIT.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed) {
        Ok(_) => {}

        Err(_) => error!("Kernel initialization was performed more than once; this is likely a software bug."),
    }
}

static INIT_STAGES: [fn(); 9] = [
    /* serial */
    || {
        static UART: spin::Lazy<crate::memory::io::Serial> = spin::Lazy::new(|| {
            // SAFETY: Function is called only once, when the `Lazy` is initialized.
            unsafe { crate::memory::io::Serial::init() }
        });

        log::set_max_level(log::LevelFilter::Trace);
        log::set_logger(&*UART).unwrap();
    },
    /* boot info */
    || {
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
    },
    /* boot parameters */
    || {
        PARAMETERS.call_once(|| {
            let mut parameters = Parameters::default();

            if let Some(cmdline) = get_kernel_file()
                .and_then(|kernel_file| kernel_file.cmdline.to_str())
                .and_then(|cmdline_cstr| cmdline_cstr.to_str().ok())
                .map(|cmdline| cmdline.split(' '))
            {
                for parameter in cmdline {
                    match parameter.split_once(':') {
                        Some(("smp", "on")) => parameters.smp = true,
                        Some(("smp", "off")) => parameters.smp = false,

                        None if parameter == "symbolinfo" => parameters.symbolinfo = true,

                        _ => warn!("Unhandled cmdline parameter: {:?}", parameter),
                    }
                }
            }

            parameters
        });

        debug!("Kernel parameters: {:?}", get_parameters());
    },
    /* memory */
    || {
        use crate::{
            memory,
            memory::{Mapper, PageAttributes, PageTable},
        };
        use libcommon::{LinkerSymbol, PageAlign};

        let memory_map = get_memory_map().unwrap();

        extern "C" {
            static __text_start: LinkerSymbol;
            static __text_end: LinkerSymbol;

            static __rodata_start: LinkerSymbol;
            static __rodata_end: LinkerSymbol;

            static __bss_start: LinkerSymbol;
            static __bss_end: LinkerSymbol;

            static __data_start: LinkerSymbol;
            static __data_end: LinkerSymbol;

            static __section_align: LinkerSymbol;
        }

        // TODO constant value for minimum page size
        //let boot_page_table = PageTable::new(4, hhdm_address, entry)

        fn map_range_from(
            from_mapper: &Mapper,
            to_mapper: &Mapper,
            range: core::ops::Range<u64>,
            attributes: PageAttributes,
        ) {
            // SAFETY: Linker should have correctly set this value.
            let page_align = unsafe { PageAlign::from_u64(__section_align.as_u64()).unwrap() };

            for address in range.step_by(page_align.as_usize()).map(Address::<Virtual>::new_truncate) {
                to_mapper
                    .map(
                        Address::<Page>::new_truncate(address, page_align),
                        // Properly handle the bootloader possibly not using huge page mappings (but still physically contiguous).
                        Address::<Page>::new(address, None).and_then(|page| from_mapper.get_mapped_to(page)).unwrap(),
                        false,
                        attributes,
                    )
                    .unwrap();
            }
        }

        // TODO don't unwrap, possibly switch to a simpler allocator? bump allocator, maybe
        // SAFETY: Kernel guarantees the HHDM to be valid.
        libcommon::memory::set({
            static KERNEL_ALLOCATOR: spin::Once<memory::slab::SlabAllocator<'static>> = spin::Once::new();

            KERNEL_ALLOCATOR.call_once(|| unsafe {
                memory::slab::SlabAllocator::from_memory_map(memory_map, memory::get_hhdm_address()).unwrap()
            })
        });

        debug!("Initializing kernel mapper...");
        // SAFETY: Kernel guarantees HHDM address to be valid.
        let boot_mapper = unsafe { Mapper::from_current(memory::get_hhdm_address()) };
        let kernel_mapper = memory::get_kernel_mapper();

        /* map the kernel segments */
        {
            map_range_from(
                &boot_mapper,
                &kernel_mapper,
                // SAFETY: These linker symbols are guaranteed by the bootloader to be valid.
                unsafe { __text_start.as_u64()..__text_end.as_u64() },
                PageAttributes::RX | PageAttributes::GLOBAL,
            );
            map_range_from(
                &boot_mapper,
                &kernel_mapper,
                // SAFETY: These linker symbols are guaranteed by the bootloader to be valid.
                unsafe { __rodata_start.as_u64()..__rodata_end.as_u64() },
                PageAttributes::RO | PageAttributes::GLOBAL,
            );
            map_range_from(
                &boot_mapper,
                &kernel_mapper,
                // SAFETY: These linker symbols are guaranteed by the bootloader to be valid.
                unsafe { __bss_start.as_u64()..__bss_end.as_u64() },
                PageAttributes::RW | PageAttributes::GLOBAL,
            );
            map_range_from(
                &boot_mapper,
                &kernel_mapper,
                // SAFETY: These linker symbols are guaranteed by the bootloader to be valid.
                unsafe { __data_start.as_u64()..__data_end.as_u64() },
                PageAttributes::RW | PageAttributes::GLOBAL,
            );
        }

        for entry in memory_map {
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
                kernel_mapper
                    .map(
                        Address::<Page>::new(
                            Address::<Virtual>::new(crate::memory::get_hhdm_address().as_u64() + phys_base).unwrap(),
                            Some(PageAlign::Align4KiB),
                        )
                        .unwrap(),
                        Address::<Frame>::new(phys_base).unwrap(),
                        false,
                        page_attributes,
                    )
                    .unwrap();
            }
        }

        debug!("Switching to kernel page tables...");
        // SAFETY: Kernel mapper has mapped all existing memory references, so commiting
        //         changes nothing from the software perspective.
        unsafe { kernel_mapper.commit_vmem_register() }.unwrap();
        debug!("Kernel has finalized control of page tables.");
    },
    /* cpu structures */
    || {
        // SAFETY: Function is called only once for BSP.
        #[cfg(target_arch = "x86_64")]
        unsafe {
            libarch::x64::cpu::init()
        };
    },
    /* acpi */
    || {
        debug!("Initializing ACPI interface...");

        static LIMINE_RSDP: limine::LimineRsdpRequest = limine::LimineRsdpRequest::new(crate::LIMINE_REV);

        match LIMINE_RSDP.get_response().get().and_then(|response| response.address.as_ptr()).map(|ptr| ptr.addr()) {
            // SAFETY: Bootloader guarantees that, if provided, the RSDP address will be valid.
            Some(address) => unsafe {
                crate::acpi::init_interface(libcommon::Address::<libcommon::Physical>::new_truncate({
                    // Properly handle the bootloader's mapping of ACPI addresses in lower-half or higher-half memory space.
                    core::cmp::min(address, address.wrapping_sub(crate::memory::get_hhdm_address().as_usize())) as u64
                }))
            },

            None => warn!("No ACPI interface identified. System functionality will be impaired."),
        }
    },
    /* interrupts */
    || {
        #[cfg(target_arch = "x86_64")]
        {
            debug!("Initializing APIC interface...");
            libarch::x64::structures::apic::init_apic(|address| {
                let page_address = Address::<Page>::new(
                    Address::<Virtual>::new(crate::memory::get_hhdm_address().as_u64() + (address as u64)).unwrap(),
                    Some(libcommon::PageAlign::Align4KiB),
                )
                .unwrap();

                crate::memory::get_kernel_mapper()
                    .map_if_not_mapped(
                        page_address,
                        Some((Address::<libcommon::Frame>::new(address as u64).unwrap(), false)),
                        crate::memory::PageAttributes::MMIO,
                    )
                    .unwrap();

                // SAFETY: TODO
                unsafe { page_address.address().as_mut_ptr() }
            })
            .unwrap();
        }
    },
    /* symbols */
    || {
        let (kernel_file_base, kernel_file_len) = {
            let kernel_file = get_kernel_file();
            (kernel_file.unwrap().base.as_ptr().unwrap(), kernel_file.unwrap().length as usize)
        };

        // SAFETY: Kernel file is guaranteed to be valid by bootloader.
        let kernel_elf =
            crate::elf::Elf::from_bytes(&(unsafe { core::slice::from_raw_parts(kernel_file_base, kernel_file_len) }))
                .expect("failed to parse kernel executable");
        if let Some(names_section) = kernel_elf.get_section_names_section() {
            for (section, name) in kernel_elf.iter_sections().filter_map(|section| {
                let names_section_offset = section.get_names_section_offset();
                Some((
                    section,
                    core::ffi::CStr::from_bytes_until_nul(&names_section.data()[names_section_offset..])
                        .ok()?
                        .to_str()
                        .ok()?,
                ))
            }) {
                {
                    use alloc::vec::Vec;

                    match name {
                        ".symtab" if let Ok(symbols) = bytemuck::try_cast_slice(section.data()) => {
                            crate::panic::KERNEL_SYMBOLS.call_once(|| {
                                let mut symbols_copy = Vec::new();
                                symbols_copy.extend_from_slice(symbols);
                                symbols_copy
                            });
                        }

                        ".strtab" => {
                            crate::panic::KERNEL_STRINGS.call_once(|| {
                                let mut strings_copy = Vec::new();
                                strings_copy.extend_from_slice(section.data());
                                strings_copy
                            });
                        }

                        _ => {}
                    }
                }
            }
        }
    },
    /* smp */
    || {
        static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(crate::LIMINE_REV).flags(0b1);

        if let Some(smp_response) = LIMINE_SMP.get_response().get_mut() {
            let bsp_lapic_id = smp_response.bsp_lapic_id;
            debug!("Detected {} additional cores.", smp_response.cpu_count - 1);
            for cpu_info in smp_response.cpus().iter_mut().filter(|info| info.lapic_id != bsp_lapic_id) {
                trace!("Starting processor: ID P{}/L{}", cpu_info.processor_id, cpu_info.lapic_id);

                cpu_info.goto_address = if get_parameters().smp {
                    extern "C" fn _smp_entry(info: *const limine::LimineSmpInfo) -> ! {
                        // SAFETY: Function is called only once per core.
                        #[cfg(target_arch = "x86_64")]
                        unsafe {
                            libarch::x64::cpu::init()
                        };

                        // SAFETY: All currently referenced memory should also be mapped in the kernel page tables.
                        unsafe { crate::memory::get_kernel_mapper().commit_vmem_register().unwrap() };

                        // SAFETY: Function is called only once for this core.
                        unsafe { crate::kernel_thread_setup(info.read().lapic_id) }
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
    },
];
