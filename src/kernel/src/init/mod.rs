mod drivers;

use libkernel::LinkerSymbol;
use libsys::Address;

pub static KERNEL_HANDLE: spin::Lazy<uuid::Uuid> = spin::Lazy::new(|| uuid::Uuid::new_v4());

/// ### Safety
///
/// This function should only ever be called by the bootloader.
#[no_mangle]
#[doc(hidden)]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    // Logging isn't set up, so we'll just spin loop if we fail to initialize it.
    crate::logging::init().unwrap_or_else(|_| crate::interrupts::wait_loop());

    /* misc. boot info */
    {
        static LIMINE_INFO: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(crate::boot::LIMINE_REV);

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

        // Vendor strings from the CPU need to be enumerated per-platform.
        #[cfg(target_arch = "x86_64")]
        if let Some(vendor_info) = crate::arch::x64::cpuid::VENDOR_INFO.as_ref() {
            info!("Vendor              {}", vendor_info.as_str());
        } else {
            info!("Vendor              Unknown");
        }
    }

    crate::cpu::setup();

    /*
     * Memory
     */

    crate::memory::with_kmapper(|kmapper| {
        use crate::memory::{address_space::Mapper, hhdm_address, PageAttributes, PageDepth};

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

        debug!("Initializing kernel mapper...");

        fn map_range_from(
            from_mapper: &Mapper,
            to_mapper: &mut Mapper,
            range: core::ops::Range<usize>,
            attributes: PageAttributes,
        ) {
            trace!("{:X?} [{:?}]", range, attributes);

            for address in range.step_by(0x1000) {
                to_mapper
                    .map(
                        Address::new_truncate(address),
                        PageDepth::min(),
                        from_mapper.get_mapped_to(Address::new_truncate(address)).unwrap(),
                        false,
                        attributes,
                    )
                    .unwrap();
            }
        }

        // Safety: All parameters are provided from valid sources.
        let boot_mapper =
            unsafe { Mapper::new_unsafe(PageDepth::current(), crate::memory::PagingRegister::read().frame()) };

        /* map the kernel segments */
        map_range_from(
            &boot_mapper,
            kmapper,
            // Safety: These linker symbols are guaranteed by the bootloader to be valid.
            unsafe { __text_start.as_ptr::<u8>().addr()..__text_end.as_ptr::<u8>().addr() },
            PageAttributes::RX | PageAttributes::GLOBAL,
        );
        map_range_from(
            &boot_mapper,
            kmapper,
            // Safety: These linker symbols are guaranteed by the bootloader to be valid.
            unsafe { __rodata_start.as_ptr::<u8>().addr()..__rodata_end.as_ptr::<u8>().addr() },
            PageAttributes::RO | PageAttributes::GLOBAL,
        );
        map_range_from(
            &boot_mapper,
            kmapper,
            // Safety: These linker symbols are guaranteed by the bootloader to be valid.
            unsafe { __bss_start.as_ptr::<u8>().addr()..__bss_end.as_ptr::<u8>().addr() },
            PageAttributes::RW | PageAttributes::GLOBAL,
        );
        map_range_from(
            &boot_mapper,
            kmapper,
            // Safety: These linker symbols are guaranteed by the bootloader to be valid.
            unsafe { __data_start.as_ptr::<u8>().addr()..__data_end.as_ptr::<u8>().addr() },
            PageAttributes::RW | PageAttributes::GLOBAL,
        );

        for entry in crate::boot::get_memory_map().unwrap() {
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

            for phys_base in (entry.base..(entry.base + entry.len)).step_by(0x1000).map(|p| p as usize) {
                kmapper
                    .map(
                        Address::new_truncate(hhdm_address().get() + phys_base),
                        PageDepth::min(),
                        Address::new_truncate(phys_base as usize),
                        false,
                        page_attributes,
                    )
                    .unwrap();
            }

            // ... map architecture-specific memory ...

            #[cfg(target_arch = "x86_64")]
            {
                // map APIC ...
                let apic_address = msr::IA32_APIC_BASE::get_base_address() as usize;
                kmapper
                    .map(
                        Address::new_truncate(hhdm_address().get() + apic_address),
                        PageDepth::min(),
                        Address::new_truncate(apic_address),
                        false,
                        PageAttributes::MMIO,
                    )
                    .unwrap();
            }
        }

        debug!("Switching to kernel page tables...");
        // Safety: Kernel mappings should be identical to the bootloader mappings.
        unsafe { kmapper.swap_into() };
        debug!("Kernel has finalized control of page tables.");
    });

    debug!("Initializing ACPI interface...");
    crate::acpi::init_interface();

    /* symbols */
    if !crate::boot::PARAMETERS.low_memory {
        match load_kernel_symbols() {
            Ok(symbols) => {
                let symbols = crate::interrupts::without(|| crate::panic::KERNEL_SYMBOLS.call_once(|| symbols));
                trace!("Kernel symbols:\n{:?}", symbols);
            }

            Err(err) => {
                warn!("Failed to load kernel symbols: {:?}", err);
            }
        }
    } else {
        debug!("Kernel is running in low memory mode; stack tracing will be disabled.");
    }

    // TODO
    debug!("Unpacking kernel drivers...");
    drivers::load_drivers();

    /* smp */
    {
        static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(crate::boot::LIMINE_REV)
            // Enable x2APIC mode if available.
            .flags(0b1);

        debug!("Detecting and starting additional cores.");

        if let Some(smp_response) = LIMINE_SMP.get_response().get_mut() {
            let bsp_lapic_id = smp_response.bsp_lapic_id;
            debug!("Detected {} additional cores.", smp_response.cpu_count - 1);
            for cpu_info in smp_response.cpus().iter_mut().filter(|info| info.lapic_id != bsp_lapic_id) {
                trace!("Starting processor: ID P{}/L{}", cpu_info.processor_id, cpu_info.lapic_id);

                cpu_info.goto_address = if crate::boot::PARAMETERS.smp {
                    extern "C" fn _smp_entry(info: *const limine::LimineSmpInfo) -> ! {
                        crate::cpu::setup();

                        // Safety: All currently referenced memory should also be mapped in the kernel page tables.
                        crate::memory::with_kmapper(|kmapper| unsafe { kmapper.swap_into() });

                        // Safety: Function is called only once for this core.
                        unsafe { kernel_core_setup(info.read().lapic_id) }
                    }

                    _smp_entry
                } else {
                    extern "C" fn _idle_forever(_: *const limine::LimineSmpInfo) -> ! {
                        // Safety: Murder isn't legal. Is this?
                        unsafe { crate::interrupts::halt_and_catch_fire() }
                    }

                    _idle_forever
                };
            }
        } else {
            debug!("Bootloader has not provided any SMP information.");
        }
    }

    debug!("Reclaiming bootloader memory...");
    crate::boot::reclaim_boot_memory({
        extern "C" {
            static __symbols_start: LinkerSymbol;
            static __symbols_end: LinkerSymbol;
        }

        &[__symbols_start.as_usize()..__symbols_end.as_usize()]
    });
    debug!("Bootloader memory reclaimed.");

    kernel_core_setup(0)
}

/// ### Safety
///
/// This function should only ever be called once per core.
#[inline(never)]
pub(self) unsafe fn kernel_core_setup(core_id: u32) -> ! {
    crate::local_state::init(core_id, 1000);

    // Ensure we enable interrupts prior to enabling the scheduler.
    crate::interrupts::enable();
    crate::local_state::begin_scheduling();

    // This interrupt wait loop is necessary to ensure the core can jump into the scheduler.
    crate::interrupts::wait_loop()
}

fn load_kernel_symbols() -> Result<&'static [(&'static str, elf::symbol::Symbol)], elf::parse::ParseError> {
    debug!("Loading kernel symbols...");

    let (kernel_file_base, kernel_file_len) = {
        let kernel_file = crate::boot::get_kernel_file().expect("failed to get kernel file");
        (kernel_file.base.as_ptr().unwrap(), kernel_file.length as usize)
    };

    // Safety: Kernel file is guaranteed to be valid by bootloader.
    let kernel_elf_data = unsafe { core::slice::from_raw_parts(kernel_file_base, kernel_file_len) };
    let kernel_elf = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(kernel_elf_data)?;
    let (symbol_table, string_table) = kernel_elf.symbol_table()?.expect("kernel file has no symbol table");

    let mut vec = try_alloc::vec::TryVec::with_capacity_in(symbol_table.len(), &*crate::memory::PMM)
        .expect("failed to allocate vector for kernel symbols");

    symbol_table.into_iter().for_each(|symbol| {
        vec.push((string_table.get(symbol.st_name as usize).unwrap_or("Unidentified"), symbol)).unwrap()
    });

    Ok(alloc::vec::Vec::leak(vec.into_vec()))
}

/* load driver */

// Push ELF as global task.

// let stack_address = {
//     const TASK_STACK_BASE_ADDRESS: Address<Page> = Address::<Page>::new_truncate(
//         Address::<Virtual>::new_truncate(128 << 39),
//         Some(PageAlign::Align2MiB),
//     );
//     // TODO make this a dynamic configuration
//     const TASK_STACK_PAGE_COUNT: usize = 2;

//     for page in (0..TASK_STACK_PAGE_COUNT)
//         .map(|offset| TASK_STACK_BASE_ADDRESS.forward_checked(offset).unwrap())
//     {
//         driver_page_manager
//             .map(
//                 page,
//                 Address::<Frame>::zero(),
//                 false,
//                 PageAttributes::WRITABLE
//                     | PageAttributes::NO_EXECUTE
//                     | PageAttributes::DEMAND
//                     | PageAttributes::USER
//                     | PageAttributes::HUGE,
//             )
//             .unwrap();
//     }

//     TASK_STACK_BASE_ADDRESS.forward_checked(TASK_STACK_PAGE_COUNT).unwrap()
// };

// TODO
// let task = crate::local_state::Task::new(
//     u8::MIN,
//     // TODO account for memory base when passing entry offset
//     crate::local_state::EntryPoint::Address(
//         Address::<Virtual>::new(elf.get_entry_offset() as u64).unwrap(),
//     ),
//     stack_address.address(),
//     {
//         #[cfg(target_arch = "x86_64")]
//         {
//             (
//                 crate::arch::x64::registers::GeneralRegisters::empty(),
//                 crate::arch::x64::registers::SpecialRegisters::flags_with_user_segments(
//                     crate::arch::x64::registers::RFlags::INTERRUPT_FLAG,
//                 ),
//             )
//         }
//     },
//     #[cfg(target_arch = "x86_64")]
//     {
//         // TODO do not error here ?
//         driver_page_manager.read_vmem_register().unwrap()
//     },
// );

// crate::local_state::queue_task(task);
