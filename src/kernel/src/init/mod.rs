mod drivers;

use libkernel::LinkerSymbol;
use libsys::{page_size, Address};

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

    {
        static LIMINE_KERNEL_ADDR: limine::LimineKernelAddressRequest =
            limine::LimineKernelAddressRequest::new(crate::boot::LIMINE_REV);
        static LIMINE_KERNEL_FILE: limine::LimineKernelFileRequest =
            limine::LimineKernelFileRequest::new(crate::boot::LIMINE_REV);

        extern "C" {
            static KERN_BASE: LinkerSymbol;
        }

        // Extract kernel address information.
        let (kernel_paddr, kernel_vaddr) = LIMINE_KERNEL_ADDR
            .get_response()
            .get()
            .map(|response| (response.physical_base as usize, response.virtual_base as usize))
            .expect("bootloader did not provide kernel address info");
        // Take reference to kernel file data.
        let kernel_file = LIMINE_KERNEL_FILE
            .get_response()
            .get()
            .and_then(|response| response.kernel_file.get())
            .expect("bootloader did not provide kernel file data");

        // Safety: Bootloader guarantees the provided information to be correct.
        let kernel_elf = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(unsafe {
            core::slice::from_raw_parts(kernel_file.base.as_ptr().unwrap(), kernel_file.length as usize)
        })
        .expect("kernel file is not a valid ELF");

        /* load and map segments */

        crate::memory::with_kmapper(|kmapper| {
            use crate::memory::{hhdm_address, PageAttributes, PageDepth};
            use limine::LimineMemoryMapEntryType;

            const PT_LOAD: u32 = 0x1;
            const PT_FLAG_EXEC_BIT: usize = 0;
            const PT_FLAG_WRITE_BIT: usize = 1;

            /* load kernel segments */
            kernel_elf
                .segments()
                .expect("kernel file has no segments")
                .into_iter()
                .filter(|ph| ph.p_type == PT_LOAD)
                .for_each(|phdr| {
                    use bit_field::BitField;

                    trace!("{:X?}", phdr);

                    let base_offset = (phdr.p_vaddr as usize) - KERN_BASE.as_usize();
                    let offset_end = base_offset + (phdr.p_memsz as usize);
                    let page_attributes = {
                        if phdr.p_flags.get_bit(PT_FLAG_EXEC_BIT) {
                            PageAttributes::RX
                        } else if phdr.p_flags.get_bit(PT_FLAG_WRITE_BIT) {
                            PageAttributes::RW
                        } else {
                            PageAttributes::RO
                        }
                    };

                    (base_offset..offset_end)
                        .step_by(page_size().get())
                        // Tuple the memory offset to the respect physical and virtual addresses.
                        .map(|mem_offset| {
                            (
                                Address::new(kernel_paddr + mem_offset).unwrap(),
                                Address::new(kernel_vaddr + mem_offset).unwrap(),
                            )
                        })
                        // Attempt to map the page to the frame.
                        .try_for_each(|(paddr, vaddr)| {
                            trace!("Map   paddr: {:X?}   vaddr: {:X?}   attrs {:?}", paddr, vaddr, page_attributes);
                            kmapper.map(vaddr, PageDepth::min(), paddr, false, page_attributes)
                        })
                        .expect("failed to map kernel segments");
                });

            /* map the higher-half direct map */
            debug!("Mapping the higher-half direct map.");
            crate::boot::get_memory_map()
                .expect("bootloader memory map is required to map HHDM")
                .iter()
                // Filter bad memory, or provide the entry's page attributes.
                .filter_map(|entry| {
                    match entry.typ {
                    LimineMemoryMapEntryType::Usable
                            | LimineMemoryMapEntryType::AcpiNvs
                            | LimineMemoryMapEntryType::AcpiReclaimable
                            | LimineMemoryMapEntryType::BootloaderReclaimable
                            // TODO handle the PATs or something to make this WC
                            | LimineMemoryMapEntryType::Framebuffer => Some((entry, PageAttributes::RW)),

                            LimineMemoryMapEntryType::Reserved | LimineMemoryMapEntryType::KernelAndModules => {
                                Some((entry, PageAttributes::RO))
                            }

                            LimineMemoryMapEntryType::BadMemory => None,
                        }
                })
                // Flatten the enumeration of every page in the entry.
                .flat_map(|(entry, attributes)| {
                    (entry.base..(entry.base + entry.len))
                        .step_by(page_size().get())
                        .map(move |phys_base| (phys_base as usize, attributes))
                })
                // Attempt to map each of the entry's pages.
                .try_for_each(|(phys_base, attributes)| {
                    kmapper.map(
                        Address::new_truncate(hhdm_address().get() + phys_base),
                        PageDepth::min(),
                        Address::new_truncate(phys_base),
                        false,
                        attributes,
                    )
                })
                .expect("failed mapping the HHDM");

            /* map architecture-specific memory */
            debug!("Mapping the architecture-specific memory.");
            #[cfg(target_arch = "x86_64")]
            {
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

            debug!("Switching to kernel page tables...");
            // Safety: Kernel mappings should be identical to the bootloader mappings.
            unsafe { kmapper.swap_into() };
            debug!("Kernel has finalized control of page tables.");
        });

        /* load symbols */
        if !crate::boot::PARAMETERS.low_memory {
            if let Ok(Some((symbol_table, string_table))) = kernel_elf.symbol_table() {
                let mut vec = try_alloc::vec::TryVec::with_capacity_in(symbol_table.len(), &*crate::memory::PMM)
                    .expect("failed to allocate vector for kernel symbols");

                symbol_table.into_iter().for_each(|symbol| {
                    vec.push((string_table.get(symbol.st_name as usize).unwrap_or("Unidentified"), symbol)).unwrap()
                });
                let symbols = alloc::vec::Vec::leak(vec.into_vec());
                trace!("Kernel symbols:\n{:?}", symbols);

                crate::interrupts::without(|| crate::panic::KERNEL_SYMBOLS.call_once(|| symbols));
            } else {
                warn!("Failed to load any kernel symbols; stack tracing will be disabled.");
            }
        } else {
            debug!("Kernel is running in low memory mode; stack tracing will be disabled.");
        }
    }

    debug!("Initializing ACPI interface...");
    crate::acpi::init_interface();

    /* symbols */

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
