mod params;

pub fn get_parameters() -> &'static params::Parameters {
    params::PARAMETERS.get().expect("parameters have not yet been parsed")
}

use libkernel::LinkerSymbol;
use libsys::{page_size, Address};

use crate::memory::{address_space::{AddressSpace, mapper::Mapper}, PageDepth};

pub static KERNEL_HANDLE: spin::Lazy<uuid::Uuid> = spin::Lazy::new(uuid::Uuid::new_v4);

/// ### Safety
///
/// This function should only ever be called by the bootloader.
#[no_mangle]
#[doc(hidden)]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    if cfg!(debug_assertions) {
        // Logging isn't set up, so we'll just spin loop if we fail to initialize it.
        crate::logging::init().unwrap_or_else(|_| crate::interrupts::wait_loop());
    } else {
        // Logging failed to initialize, but just continue to boot (only in release).
        crate::logging::init().ok();
    }

    /* misc. boot info */
    {
        #[limine::limine_tag]
        static BOOT_INFO: limine::BootInfoRequest = limine::BootInfoRequest::new(crate::boot::LIMINE_REV);

        if let Some(boot_info) = BOOT_INFO.get_response() {
            info!("Bootloader Info     {} v{} (rev {})", boot_info.name(), boot_info.version(), boot_info.revision());
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
        #[limine::limine_tag]
        static LIMINE_KERNEL_ADDR: limine::KernelAddressRequest =
            limine::KernelAddressRequest::new(crate::boot::LIMINE_REV);
        #[limine::limine_tag]
        static LIMINE_KERNEL_FILE: limine::KernelFileRequest = limine::KernelFileRequest::new(crate::boot::LIMINE_REV);

        extern "C" {
            static KERN_BASE: LinkerSymbol;
        }

        // Extract kernel address information.
        let (kernel_phys_addr, kernel_virt_addr) = LIMINE_KERNEL_ADDR
            .get_response()
            .map(|response| {
                (usize::try_from(response.physical_base()).unwrap(), usize::try_from(response.virtual_base()).unwrap())
            })
            .expect("bootloader did not provide kernel address info");

        // Take reference to kernel file data.
        let kernel_file = LIMINE_KERNEL_FILE
            .get_response()
            .map(limine::KernelFileResponse::file)
            .expect("bootloader did not provide kernel file data");

        /* parse parameters */
        params::PARAMETERS.call_once(|| params::Parameters::parse(kernel_file.cmdline()));

        // Safety: Bootloader guarantees the provided information to be correct.
        let kernel_elf = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(kernel_file.data())
            .expect("kernel file is not a valid ELF");

        /* load and map segments */

        crate::memory::with_kmapper(|kmapper| {
            use crate::memory::{hhdm_address, paging::Attributes};
            use limine::MemoryMapEntryType;

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

                    debug!("{:X?}", phdr);

                    let base_offset = usize::try_from(phdr.p_vaddr).unwrap() - KERN_BASE.as_usize();
                    let offset_end = base_offset + usize::try_from(phdr.p_memsz).unwrap();
                    let page_attributes = {
                        if phdr.p_flags.get_bit(PT_FLAG_EXEC_BIT) {
                            Attributes::RX
                        } else if phdr.p_flags.get_bit(PT_FLAG_WRITE_BIT) {
                            Attributes::RW
                        } else {
                            Attributes::RO
                        }
                    };

                    (base_offset..offset_end)
                        .step_by(page_size())
                        // Tuple the memory offset to the respect physical and virtual addresses.
                        .map(|mem_offset| {
                            (
                                Address::new(kernel_phys_addr + mem_offset).unwrap(),
                                Address::new(kernel_virt_addr + mem_offset).unwrap(),
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
                    match entry.ty() {
                    MemoryMapEntryType::Usable
                            | MemoryMapEntryType::AcpiNvs
                            | MemoryMapEntryType::AcpiReclaimable
                            | MemoryMapEntryType::BootloaderReclaimable
                            // TODO handle the PATs or something to make this WC
                            | MemoryMapEntryType::Framebuffer => Some((entry, Attributes::RW)),

                            MemoryMapEntryType::Reserved | MemoryMapEntryType::KernelAndModules => {
                                Some((entry, Attributes::RO))
                            }

                            MemoryMapEntryType::BadMemory => None,
                        }
                })
                // Flatten the enumeration of every page in the entry.
                .flat_map(|(entry, attributes)| {
                    entry.range().step_by(page_size()).map(move |phys_base| (phys_base.try_into().unwrap(), attributes))
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
                let apic_address = msr::IA32_APIC_BASE::get_base_address().try_into().unwrap();
                kmapper
                    .map(
                        Address::new_truncate(hhdm_address().get() + apic_address),
                        PageDepth::min(),
                        Address::new_truncate(apic_address),
                        false,
                        Attributes::MMIO,
                    )
                    .unwrap();
            }

            debug!("Switching to kernel page tables...");
            // Safety: Kernel mappings should be identical to the bootloader mappings.
            unsafe { kmapper.swap_into() };
            debug!("Kernel has finalized control of page tables.");
        });

        /* load symbols */
        if get_parameters().low_memory {
            debug!("Kernel is running in low memory mode; stack tracing will be disabled.");
        } else if let Ok(Some((symbol_table, string_table))) = kernel_elf.symbol_table() {
            let mut vec = try_alloc::vec::TryVec::with_capacity_in(symbol_table.len(), &*crate::memory::PMM)
                .expect("failed to allocate vector for kernel symbols");

            symbol_table.into_iter().for_each(|symbol| {
                vec.push((string_table.get(symbol.st_name as usize).unwrap_or("Unidentified"), symbol)).unwrap();
            });
            crate::interrupts::without(|| {
                crate::panic::KERNEL_SYMBOLS.call_once(|| alloc::vec::Vec::leak(vec.into_vec()))
            });
        } else {
            warn!("Failed to load any kernel symbols; stack tracing will be disabled.");
        }
    }

    debug!("Initializing ACPI interface...");
    crate::acpi::init_interface();

    /* load drivers */
    {
        use crate::proc::{task::{Task, EntryPoint},};
        use elf::{endian::AnyEndian, ElfBytes};

        #[limine::limine_tag]
        static LIMINE_MODULES: limine::ModuleRequest = limine::ModuleRequest::new(crate::boot::LIMINE_REV);

        debug!("Unpacking kernel drivers...");

        if let Some(modules) = LIMINE_MODULES.get_response() {
            for module in modules
                .modules()
                .iter()
                // Filter out modules that don't end with our driver postfix.
                .filter(|module| module.path().ends_with("drivers"))
            {
                let archive = tar_no_std::TarArchiveRef::new(module.data());
                for entry in archive.entries() {
                    debug!("Attempting to parse driver blob: {}", entry.filename());

                    let Ok(elf) = ElfBytes::<AnyEndian>::minimal_parse(entry.data()) 
                    else {
                        warn!("Failed to parse driver blob into ELF");
                        continue;
                    };

                    let entry_point = core::mem::transmute::<_, EntryPoint>(elf.ehdr.e_entry);
                    let address_space = AddressSpace::new(crate::memory::address_space::DEFAULT_USERSPACE_SIZE, Mapper::new_unsafe(PageDepth::current(), crate::memory::new_kmapped_page_table().unwrap()), &*crate::memory::PMM);
                    let task = Task::new(0, entry_point, address_space, crate::cpu::ArchContext::user_default());

                    crate::proc::PROCESSES.lock().push_back(task);
                }
            }
        } else {
            error!("Bootloader did not provide an init module.");
        };

        // for (entry, mapper) in artifacts.into_iter().map(Artifact::decompose) {
        //     let task = Task::new(0, entry, stack, crate::cpu::ArchContext::user_context())
        // }
    }

    /* smp */
    {
        #[limine::limine_tag]
        static LIMINE_SMP: limine::SmpRequest = limine::SmpRequest::new(crate::boot::LIMINE_REV)
            // Enable x2APIC mode if available.
            .flags(0b1);

        // Safety: `LIMINE_SMP` is only ever accessed within this individual context, and is effectively
        //          dropped as soon as this context goes out of scope.
        let limine_smp = unsafe { &mut *(&raw const LIMINE_SMP).cast_mut() };

        debug!("Detecting and starting additional cores.");

        limine_smp.get_response_mut().map(limine::SmpResponse::cpus).map_or_else(
            || debug!("Bootloader detected no additional CPU cores."),
            // Iterate all of the CPUs, and jump them to the SMP function.
            |cpus| {
                for cpu_info in cpus {
                    trace!("Starting processor: ID P{}/L{}", cpu_info.processor_id(), cpu_info.lapic_id());

                    if get_parameters().smp {
                        extern "C" fn _smp_entry(info: &limine::CpuInfo) -> ! {
                            crate::cpu::setup();

                            // Safety: All currently referenced memory should also be mapped in the kernel page tables.
                            crate::memory::with_kmapper(|kmapper| unsafe { kmapper.swap_into() });

                            // Safety: Function is called only once for this core.
                            unsafe { kernel_core_setup(info.lapic_id()) }
                        }

                        // If smp is enabled, jump to the smp entry function.
                        cpu_info.jump_to(_smp_entry, None);
                    } else {
                        extern "C" fn _idle_forever(_: &limine::CpuInfo) -> ! {
                            // Safety: Murder isn't legal. Is this?
                            unsafe { crate::interrupts::halt_and_catch_fire() }
                        }

                        // If smp is disabled, jump to the park function for the core.
                        cpu_info.jump_to(_idle_forever, None);
                    }
                }
            },
        );
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
