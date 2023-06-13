mod params;
pub use params::*;

use crate::mem::{alloc::AlignedAllocator, paging::TableDepth};
use core::mem::MaybeUninit;
use libkernel::LinkerSymbol;
use libsys::{page_size, Address};

pub static KERNEL_HANDLE: spin::Lazy<uuid::Uuid> = spin::Lazy::new(uuid::Uuid::new_v4);

#[allow(clippy::too_many_lines)]
pub unsafe extern "C" fn init() -> ! {
    use core::sync::atomic::{AtomicBool, Ordering};

    static HAS_INIT: AtomicBool = AtomicBool::new(false);

    if HAS_INIT.compare_exchange(false, true, Ordering::Release, Ordering::Relaxed).is_err() {
        panic!("`init()` has already been called!");
    } else {
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
}

/// ### Safety
///
/// This function should only ever be called once per core.
pub(self) unsafe fn kernel_core_setup() -> ! {
    crate::local::init(1000);

    // Ensure we enable interrupts prior to enabling the scheduler.
    crate::interrupts::enable();
    crate::local::begin_scheduling().unwrap();

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
    #[limine::limine_tag]
    static BOOT_INFO: limine::BootInfoRequest = limine::BootInfoRequest::new(crate::boot::LIMINE_REV);

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

    debug!("Preparing kernel memory system.");

    // Extract kernel address information.
    let (kernel_phys_addr, kernel_virt_addr) = LIMINE_KERNEL_ADDR
        .get_response()
        .map(|response| {
            (usize::try_from(response.physical_base()).unwrap(), usize::try_from(response.virtual_base()).unwrap())
        })
        .expect("bootloader did not provide kernel address info");

    // Take reference to kernel file data.
    let kernel_file = crate::boot::kernel_file().expect("bootloader provided no kernel file");

    // Safety: Bootloader guarantees the provided information to be correct.
    let kernel_elf = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(kernel_file.data())
        .expect("kernel file is not a valid ELF");

    /* load and map segments */

    crate::mem::with_kmapper(|kmapper| {
        use crate::mem::{paging::TableEntryFlags, HHDM};
        use limine::MemoryMapEntryType;

        debug!("Mapping the higher-half direct map.");
        crate::boot::get_memory_map()
            .expect("bootloader memory map is required to map HHDM")
            .iter()
            .map(|entry| (entry.range(), entry.ty()))
            .enumerate()
            .cycle()
            .try_reduce(|(last_index, (last_range, last_ty)), (index, (range, ty))| {
                use core::ops::ControlFlow;

                if last_range.end == range.start && last_ty == ty {
                    ControlFlow::Continue((index, (last_range.start..range.end, last_ty)))
                } else {
                    let mmap_args = {
                        match last_ty {
                            MemoryMapEntryType::Usable => Some((TableEntryFlags::RW, false)),

                            MemoryMapEntryType::AcpiNvs
                            | MemoryMapEntryType::AcpiReclaimable
                            | MemoryMapEntryType::BootloaderReclaimable
                            | MemoryMapEntryType::Framebuffer => Some((TableEntryFlags::RW, true)),

                            MemoryMapEntryType::Reserved | MemoryMapEntryType::KernelAndModules => {
                                Some((TableEntryFlags::RO, true))
                            }

                            MemoryMapEntryType::BadMemory => None,
                        }
                    };

                    if let Some((flags, lock_frame)) = mmap_args {
                        const HUGE_PAGE_DEPTH: TableDepth = TableDepth::new_mappable::<1>().unwrap();

                        trace!(
                            "HHDM Map({})  {:#X?}  {:?} -> {:?}   lock {}",
                            index,
                            last_range,
                            last_ty,
                            flags,
                            lock_frame
                        );

                        let mut region_range =
                            usize::try_from(last_range.start).unwrap()..usize::try_from(last_range.end).unwrap();

                        while !region_range.is_empty() {
                            if region_range.len() > HUGE_PAGE_DEPTH.align()
                                && region_range.start.trailing_zeros() >= HUGE_PAGE_DEPTH.align().trailing_zeros()
                            {
                                let frame = Address::new(region_range.start).unwrap();
                                let page = HHDM.offset(frame).unwrap();
                                region_range.advance_by(HUGE_PAGE_DEPTH.align()).unwrap();

                                kmapper
                                    .map(page, HUGE_PAGE_DEPTH, frame, lock_frame, flags | TableEntryFlags::HUGE)
                                    .expect("failed multi-page HHDM mapping");
                            } else {
                                let frame = Address::new(region_range.start).unwrap();
                                let page = HHDM.offset(frame).unwrap();
                                region_range.advance_by(page_size()).unwrap();

                                kmapper
                                    .map(page, TableDepth::min(), frame, lock_frame, flags)
                                    .expect("failed single page HHDM mapping");
                            }
                        }
                    } else {
                        trace!("HHDM Map (!! bad memory !!) @{:#X?}", last_range);
                    }

                    if last_index < index {
                        ControlFlow::Continue((index, (range, ty)))
                    } else {
                        ControlFlow::Break(())
                    }
                }
            });

        /* load kernel segments */
        kernel_elf
            .segments()
            .expect("kernel file has no segments")
            .into_iter()
            .filter(|ph| ph.p_type == elf::abi::PT_LOAD)
            .for_each(|phdr| {
                extern "C" {
                    static KERNEL_BASE: LinkerSymbol;
                }

                debug!("{:X?}", phdr);

                // Safety: `KERNEL_BASE` is a linker symbol to an in-executable memory location, so it is guaranteed to be valid (and is never written to).
                let base_offset = usize::try_from(phdr.p_vaddr).unwrap() - unsafe { KERNEL_BASE.as_usize() };
                let base_offset_end = base_offset + usize::try_from(phdr.p_memsz).unwrap();
                let flags = TableEntryFlags::from(crate::task::segment_type_to_mmap_permissions(phdr.p_flags));

                (base_offset..base_offset_end)
                    .step_by(page_size())
                    // Attempt to map the page to the frame.
                    .try_for_each(|offset| {
                        let phys_addr = Address::new(kernel_phys_addr + offset).unwrap();
                        let virt_addr = Address::new(kernel_virt_addr + offset).unwrap();

                        trace!("Map  {:X?} -> {:X?}   {:?}", virt_addr, phys_addr, flags);
                        kmapper.map(virt_addr, TableDepth::min(), phys_addr, true, flags)
                    })
                    .expect("failed to map kernel segments");
            });

        debug!("Switching to kernel page tables...");
        // Safety: Kernel mappings should be identical to the bootloader mappings.
        unsafe { kmapper.swap_into() };
        debug!("Kernel has finalized control of page tables.");
    });
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

            trace!("Copying ELF data into memory...");
            // Safety: In-place transmutation of initialized bytes for the purpose of copying safely.
            let archive_data = unsafe { entry.data().align_to::<MaybeUninit<u8>>().1 };
            // Allocate space for the ELF data, properly aligned in memory.
            let mut elf_copy = crate::task::ElfMemory::new_zeroed_slice_in(archive_data.len(), AlignedAllocator::new());
            // Copy the ELF data from the archive entry.
            elf_copy.copy_from_slice(archive_data);
            trace!("ELF data copied into memory.");

            let (Ok((Some(shdrs), Some(_))), Ok(Some((_, _)))) = (elf.section_headers_with_strtab(), elf.symbol_table())
            else {
                panic!("Error retrieving ELF relocation metadata.")
            };

            let load_offset = crate::task::MIN_LOAD_OFFSET;

            trace!("Processing relocations localized to fault page.");
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
                // Safety: The ELF data buffer is now initialized with the contents of the ELF.
                crate::task::ElfData::Memory(unsafe { elf_copy.assume_init() }),
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

                if PARAMETERS.smp {
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
