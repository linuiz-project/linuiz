mod params;

pub fn get_parameters() -> &'static params::Parameters {
    params::PARAMETERS.get().expect("parameters have not yet been parsed")
}

use crate::mem::{alloc::AlignedAllocator, paging::PageDepth};
use core::mem::MaybeUninit;
use libkernel::LinkerSymbol;
use libsys::{page_size, Address};

pub static KERNEL_HANDLE: spin::Lazy<uuid::Uuid> = spin::Lazy::new(uuid::Uuid::new_v4);

/// ### Safety
///
/// This function should only ever be called by the bootloader.
#[no_mangle]
#[doc(hidden)]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    setup_logging();

    print_boot_info();

    crate::cpu::setup();

    setup_memory();

    debug!("Initializing ACPI interface...");
    crate::acpi::init_interface();

    load_drivers();

    setup_smp();

    debug!("Reclaiming bootloader memory...");
    crate::boot::reclaim_boot_memory({
        extern "C" {
            static __symbols_start: LinkerSymbol;
            static __symbols_end: LinkerSymbol;
        }

        &[__symbols_start.as_usize()..__symbols_end.as_usize()]
    });
    debug!("Bootloader memory reclaimed.");

    kernel_core_setup()
}

/// ### Safety
///
/// This function should only ever be called once per core.
#[inline(never)]
pub(self) unsafe fn kernel_core_setup() -> ! {
    crate::local::init(1000);

    // Ensure we enable interrupts prior to enabling the scheduler.
    crate::interrupts::enable();
    crate::local::begin_scheduling();

    // This interrupt wait loop is necessary to ensure the core can jump into the scheduler.
    crate::interrupts::wait_loop()
}

fn setup_logging() {
    if cfg!(debug_assertions) {
        // Logging isn't set up, so we'll just spin loop if we fail to initialize it.
        crate::logging::init().unwrap_or_else(|_| crate::interrupts::wait_loop());
    } else {
        // Logging failed to initialize, but just continue to boot (only in release).
        crate::logging::init().ok();
    }
}

fn print_boot_info() {
    extern "C" {
        static __build_id: LinkerSymbol;
    }

    #[limine::limine_tag]
    static BOOT_INFO: limine::BootInfoRequest = limine::BootInfoRequest::new(crate::boot::LIMINE_REV);

    // TODO the printed build ID is not correct
    // Safety: Symbol is provided by linker script.
    info!("Build ID            {:X}", unsafe { __build_id.as_usize() });

    if let Some(boot_info) = BOOT_INFO.get_response() {
        info!("Bootloader Info     {} v{} (rev {})", boot_info.name(), boot_info.version(), boot_info.revision());
    } else {
        info!("No bootloader info available.");
    }

    // Vendor strings from the CPU need to be enumerated per-platform.
    #[cfg(target_arch = "x86_64")]
    if let Some(vendor_info) = crate::arch::x64::cpuid::VENDOR_INFO.as_ref() {
        info!("Vendor              {}", vendor_info.as_str());
    } else {
        info!("Vendor              Unknown");
    }
}

#[allow(clippy::too_many_lines)]
fn setup_memory() {
    #[limine::limine_tag]
    static LIMINE_KERNEL_ADDR: limine::KernelAddressRequest =
        limine::KernelAddressRequest::new(crate::boot::LIMINE_REV);
    #[limine::limine_tag]
    static LIMINE_KERNEL_FILE: limine::KernelFileRequest = limine::KernelFileRequest::new(crate::boot::LIMINE_REV);

    extern "C" {
        static KERN_BASE: LinkerSymbol;
    }

    debug!("Preparing kernel memory system.");

    crate::mem::Hhdm::initialize();

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

    crate::mem::with_kmapper(|kmapper| {
        use crate::mem::{paging::TableEntryFlags, Hhdm};
        use limine::MemoryMapEntryType;

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
                            | MemoryMapEntryType::Framebuffer => Some((entry, TableEntryFlags::RW)),

                            MemoryMapEntryType::Reserved | MemoryMapEntryType::KernelAndModules => {
                                Some((entry, TableEntryFlags::RO))
                            }

                            MemoryMapEntryType::BadMemory => None,
                        }
            })
            .try_for_each(|(mmap_entry, flags)| {
                use crate::mem::paging;
                use libsys::{page_shift, Frame};

                let huge_page_depth = PageDepth::new(1).unwrap();

                let region_range = mmap_entry.range();
                let region_address = Address::<Frame>::new(usize::try_from(region_range.start).unwrap()).unwrap();
                let region_size = usize::try_from(region_range.end - region_range.start).unwrap();
                let mut remaining_len = region_size;

                while remaining_len > 0 {
                    let offset = libsys::align_down(region_size - remaining_len, page_shift());
                    let frame = Address::new(region_address.get().get() + offset).unwrap();
                    let page = Hhdm::offset(frame).unwrap();

                    if remaining_len > huge_page_depth.align() && page.as_ptr().is_aligned_to(huge_page_depth.align()) {
                        kmapper.map(page, huge_page_depth, frame, false, flags | TableEntryFlags::HUGE)?;
                        remaining_len -= huge_page_depth.align();
                    } else {
                        kmapper.map(page, PageDepth::min(), frame, false, flags)?;
                        remaining_len -= page_size();
                    }
                }

                paging::Result::Ok(())
            })
            .expect("failed mapping the HHDM");

        /* map architecture-specific memory */
        debug!("Mapping the architecture-specific memory.");
        #[cfg(target_arch = "x86_64")]
        {
            let apic_address = msr::IA32_APIC_BASE::get_base_address().try_into().unwrap();
            kmapper
                .map(
                    Address::new_truncate(Hhdm::address().get() + apic_address),
                    PageDepth::min(),
                    Address::new_truncate(apic_address),
                    false,
                    TableEntryFlags::MMIO,
                )
                .unwrap();
        }

        /* load kernel segments */
        kernel_elf
            .segments()
            .expect("kernel file has no segments")
            .into_iter()
            .filter(|ph| ph.p_type == elf::abi::PT_LOAD)
            .for_each(|phdr| {
                debug!("{:X?}", phdr);

                // Safety: `KERNEL_BASE` is a linker symbol to an in-executable memory location, so it is guaranteed to
                //         be valid (and is never written to).
                let base_offset = usize::try_from(phdr.p_vaddr).unwrap() - unsafe { KERN_BASE.as_usize() };
                let offset_end = base_offset + usize::try_from(phdr.p_memsz).unwrap();
                let flags = TableEntryFlags::from(crate::task::segment_type_to_mmap_permissions(phdr.p_flags));

                (base_offset..offset_end)
                    .step_by(page_size())
                    // Attempt to map the page to the frame.
                    .try_for_each(|mem_offset| {
                        let phys_addr = Address::new(kernel_phys_addr + mem_offset).unwrap();
                        let virt_addr = Address::new(kernel_virt_addr + mem_offset).unwrap();

                        trace!("Map  {:X?} -> {:X?}   {:?}", virt_addr, phys_addr, flags);
                        kmapper.map(virt_addr, PageDepth::min(), phys_addr, false, flags)
                    })
                    .expect("failed to map kernel segments");
            });

        debug!("Switching to kernel page tables...");
        // Safety: Kernel mappings should be identical to the bootloader mappings.
        unsafe { kmapper.swap_into() };
        debug!("Kernel has finalized control of page tables.");
    });

    /* load symbols */
    if get_parameters().low_memory {
        debug!("Kernel is running in low memory mode; stack tracing will be disabled.");
    } else if let Ok(Some(tables)) = kernel_elf.symbol_table() {
        debug!("Loading kernel symbol table...");
        crate::panic::KERNEL_SYMBOLS.call_once(|| tables);
    } else {
        warn!("Failed to load any kernel symbols; stack tracing will be disabled.");
    }
}

fn load_drivers() {
    use crate::task::{AddressSpace, Priority, Task};
    use elf::endian::AnyEndian;

    #[limine::limine_tag]
    static LIMINE_MODULES: limine::ModuleRequest = limine::ModuleRequest::new(crate::boot::LIMINE_REV);

    debug!("Unpacking kernel drivers...");

    let Some(modules) = LIMINE_MODULES.get_response() else {
            warn!("Bootloader provided no modules; skipping driver loading.");
            return;
        };
    trace!("{:?}", modules);

    let modules = modules.modules();
    trace!("Found modules: {:X?}", modules);

    let Some(drivers_module) = modules.iter().find(|module| module.path().ends_with("drivers"))
    else {
        panic!("no drivers module found")
    };

    let archive = tar_no_std::TarArchiveRef::new(drivers_module.data());
    archive
        .entries()
        .filter_map(|entry| {
            debug!("Attempting to parse driver blob: {}", entry.filename());

            match elf::ElfBytes::<AnyEndian>::minimal_parse(entry.data()) {
                Ok(elf) => Some((entry, elf)),
                Err(err) => {
                    error!("Failed to parse driver blob into ELF: {:?}", err);
                    None
                }
            }
        })
        .for_each(|(entry, elf)| {
            // Get and copy the ELF segments into a small box.
            let Some(segments_copy) = elf.segments().map(|segments| segments.into_iter().collect())
            else {
                error!("ELF has no segments.");
                return
            };

            // Safety: In-place transmutation of initialized bytes for the purpose of copying safely.
            let archive_data = unsafe { entry.data().align_to::<MaybeUninit<u8>>().1 };
            // Allocate space for the ELF data, properly aligned in memory.
            let mut elf_copy = crate::task::ElfMemory::new_uninit_slice_in(archive_data.len(), AlignedAllocator::new());
            // Copy the ELF data from the archive entry.
            elf_copy.copy_from_slice(archive_data);
            // Safety: The ELF data buffer is now initialized with the contents of the ELF.
            let elf_memory = unsafe { elf_copy.assume_init() };

            let (Ok((Some(shdrs), Some(_))), Ok(Some((_, _)))) = (elf.section_headers_with_strtab(), elf.symbol_table())
            else { panic!("Error retrieving ELF relocation metadata.") };

            let load_offset = crate::task::MIN_LOAD_OFFSET;

            let relas = shdrs
                .iter()
                .filter(|shdr| shdr.sh_type == elf::abi::SHT_RELA)
                .flat_map(|shdr| elf.section_data_as_relas(&shdr).unwrap())
                .map(|rela| {
                    use crate::task::ElfRela;

                    match rela.r_type {
                        elf::abi::R_X86_64_RELATIVE => ElfRela {
                            address: Address::new(usize::try_from(rela.r_offset).unwrap()).unwrap(),
                            value: load_offset + usize::try_from(rela.r_addend).unwrap(),
                        },

                        _ => unimplemented!(),
                    }
                })
                .collect();

            let task = Task::new(
                Priority::Normal,
                AddressSpace::new_userspace(),
                load_offset,
                elf.ehdr,
                segments_copy,
                relas,
                crate::task::ElfData::Memory(elf_memory),
            );

            crate::task::PROCESSES.lock().push_back(task);
        });
}

fn setup_smp() {
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
                    extern "C" fn _smp_entry(_: &limine::CpuInfo) -> ! {
                        crate::cpu::setup();

                        // Safety: All currently referenced memory should also be mapped in the kernel page tables.
                        crate::mem::with_kmapper(|kmapper| unsafe { kmapper.swap_into() });

                        // Safety: Function is called only once for this core.
                        unsafe { kernel_core_setup() }
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
