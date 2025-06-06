use libsys::{Address, Frame, Physical, Virtual};
use limine::{
    memory_map,
    mp::RequestFlags,
    request::{
        BootloaderInfoRequest, ExecutableAddressRequest, ExecutableCmdlineRequest,
        ExecutableFileRequest, HhdmRequest, MemoryMapRequest, MpRequest, RsdpRequest,
    },
};

use crate::mem::pmm::PhysicalMemoryManager;

#[allow(clippy::too_many_lines)]
pub extern "C" fn init() -> ! {
    // This function is absolutely massive, and that's intentional. All of the code
    // within this function should be absolutely, definitely run ONLY ONCE. Writing
    // the code sequentially within one function easily ensures that will be the case.

    // All limine feature requests (ensures they are not used after bootloader memory is reclaimed)
    static BOOT_INFO_REQUEST: BootloaderInfoRequest = BootloaderInfoRequest::new();
    static KERNEL_FILE_REQUEST: ExecutableFileRequest = ExecutableFileRequest::new();
    static KERNEL_CMDLINE_REQUEST: ExecutableCmdlineRequest = ExecutableCmdlineRequest::new();
    static KERNEL_ADDR_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();
    static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
    static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
    static RSDP_ADDRESS_REQUEST: RsdpRequest = RsdpRequest::new();
    static MP_REQUEST: MpRequest = MpRequest::new().with_flags(RequestFlags::X2APIC);
    // Enable logging first, so we can get feedback on the entire init process.
    if crate::logging::UartLogger::init().is_err() {
        // Safety: Logging subsystem must be enabled to run / debug OS.
        unsafe {
            crate::interrupts::halt_and_catch_fire();
        }
    }

    // Safety: Function is run only once for this hardware thread.
    unsafe {
        #[cfg(target_arch = "x86_64")]
        crate::arch::x86_64::configure_hwthread();
    }

    if let Some(boot_info) = BOOT_INFO_REQUEST.get_response() {
        info!(
            "Bootloader Info     {} v{} (rev {})",
            boot_info.name(),
            boot_info.version(),
            boot_info.revision()
        );
    } else {
        info!("Bootloader Info     UNKNOWN");
    }

    crate::params::parse(&KERNEL_CMDLINE_REQUEST);
    crate::panic::symbols::parse(&KERNEL_FILE_REQUEST);
    crate::mem::Hhdm::init(&HHDM_REQUEST);
    crate::mem::pmm::PhysicalMemoryManager::init(&MEMORY_MAP_REQUEST);

    crate::arch::x86_64::instructions::breakpoint();

    // Set up various variables and structures for init to use.
    let kernel_file = KERNEL_FILE_REQUEST
        .get_response()
        .map(limine::response::ExecutableFileResponse::file)
        .expect("no response to kernel file request");
    // SAFETY: memory region is initialized by Limine.
    let kernel_file_mem = unsafe {
        core::slice::from_raw_parts(kernel_file.addr(), kernel_file.size().try_into().unwrap())
    };
    let kernel_elf = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(kernel_file_mem)
        .expect("failed to parse kernel file into ELF binary");
    let (kernel_addr_phys, kernel_addr_virt) = {
        let kernel_addr_response = KERNEL_ADDR_REQUEST
            .get_response()
            .expect("no kernel address response");
        (
            Address::<Physical>::new(kernel_addr_response.physical_base().try_into().unwrap())
                .unwrap(),
            Address::<Virtual>::new(kernel_addr_response.virtual_base().try_into().unwrap())
                .unwrap(),
        )
    };

    /* SETUP KERNEL MEMORY */
    // {
    //     use crate::mem::{
    //         hhdm,
    //         paging::{TableDepth, TableEntryFlags},
    //     };
    //     use libsys::page_size;
    //     use limine::memory_map::EntryType;

    //     fn map_hhdm_range(
    //         mapper: &mut crate::mem::mapper::Mapper,
    //         mut range: core::ops::Range<usize>,
    //         flags: TableEntryFlags,
    //         lock_frames: bool,
    //     ) {
    //         let huge_page_depth = TableDepth::new(1).unwrap();

    //         trace!("HHDM Map  {range:#X?}  {flags:?}   lock {lock_frames}");

    //         while !range.is_empty() {
    //             if range.len() > huge_page_depth.align()
    //                 && range.start.trailing_zeros() >= huge_page_depth.align().trailing_zeros()
    //             {
    //                 // Map a huge page

    //                 let frame = Address::new(range.start).unwrap();
    //                 let page = hhdm::get().offset(frame).unwrap();
    //                 range.advance_by(huge_page_depth.align()).unwrap();

    //                 mapper
    //                     .map(
    //                         page,
    //                         huge_page_depth,
    //                         frame,
    //                         lock_frames,
    //                         flags | TableEntryFlags::HUGE,
    //                     )
    //                     .expect("failed to map range")
    //             } else {
    //                 // Map a standard page

    //                 let frame = Address::new(range.start).unwrap();
    //                 let page = hhdm::get().offset(frame).unwrap();
    //                 range.advance_by(page_size()).unwrap();

    //                 mapper
    //                     .map(page, TableDepth::min(), frame, lock_frames, flags)
    //                     .expect("failed to map range");
    //             }
    //         }
    //     }

    //     debug!("Preparing kernel memory system.");

    //     /* load and map segments */
    //     debug!("Mapping the higher-half direct map.");
    //     crate::mem::with_kmapper(|kmapper| {
    //         let mmap_entries = &mut memory_map.iter().map(|entry| {
    //             let entry_start = usize::try_from(entry.base).unwrap();
    //             let entry_end = usize::try_from(entry.base + entry.length).unwrap();

    //             (entry_start..entry_end, entry.entry_type)
    //         });

    //         let mut last_end = 0;
    //         while let Some((mut entry_range, entry_ty)) = mmap_entries.next() {
    //             // collapse sequential matching entries
    //             if let Some((end_range, _)) = mmap_entries
    //                 .take_while(|(range, ty)| entry_range.end == range.start && entry_ty.eq(ty))
    //                 .last()
    //             {
    //                 entry_range.end = end_range.end;
    //             }

    //             if entry_range.start > last_end {
    //                 map_hhdm_range(
    //                     kmapper,
    //                     last_end..entry_range.start,
    //                     TableEntryFlags::RW,
    //                     true,
    //                 );
    //             }

    //             last_end = entry_range.end;

    //             let mmap_args = match entry_ty {
    //                 EntryType::USABLE => Some((TableEntryFlags::RW, false)),

    //                 EntryType::ACPI_NVS
    //                 | EntryType::ACPI_RECLAIMABLE
    //                 | EntryType::BOOTLOADER_RECLAIMABLE
    //                 | EntryType::FRAMEBUFFER => Some((TableEntryFlags::RW, true)),

    //                 EntryType::RESERVED | EntryType::EXECUTABLE_AND_MODULES => {
    //                     Some((TableEntryFlags::RO, true))
    //                 }

    //                 EntryType::BAD_MEMORY => None,

    //                 _ => unreachable!(),
    //             };

    //             if let Some((flags, lock_frames)) = mmap_args {
    //                 map_hhdm_range(kmapper, entry_range, flags, lock_frames);
    //             } else {
    //                 trace!("HHDM Map (!! BAD MEMORY !!) @{entry_range:#X?}");
    //             }
    //         }

    //         /* load kernel segments */
    //         kernel_elf
    //             .segments()
    //             .expect("kernel file has no segments")
    //             .into_iter()
    //             .filter(|ph| ph.p_type == elf::abi::PT_LOAD)
    //             .for_each(|phdr| {
    //                 unsafe extern "C" {
    //                     static KERNEL_BASE: libkernel::LinkerSymbol;
    //                 }

    //                 debug!("{phdr:X?}");

    //                 // Safety: `KERNEL_BASE` is a linker symbol to an in-executable memory location, so it is guaranteed to be valid (and is never written to).
    //                 let base_offset =
    //                     usize::try_from(phdr.p_vaddr).unwrap() - unsafe { KERNEL_BASE.as_usize() };
    //                 let base_offset_end = base_offset + usize::try_from(phdr.p_memsz).unwrap();
    //                 let flags = crate::mem::paging::TableEntryFlags::from(
    //                     crate::task::segment_to_mmap_permissions(phdr.p_flags),
    //                 );

    //                 (base_offset..base_offset_end)
    //                     .step_by(page_size())
    //                     // Attempt to map the page to the frame.
    //                     .for_each(|offset| {
    //                         let phys_addr = Address::new(kernel_addr_phys.get() + offset).unwrap();
    //                         let virt_addr = Address::new(kernel_addr_virt.get() + offset).unwrap();

    //                         trace!("Map  {virt_addr:X?} -> {phys_addr:X?}   {flags:?}");
    //                         kmapper
    //                             .map(virt_addr, TableDepth::min(), phys_addr, true, flags)
    //                             .expect("failed to map kernel memory region");
    //                     });
    //             });

    //         debug!("Switching to kernel page tables...");
    //         // Safety: Kernel mappings should be identical to the bootloader mappings.
    //         unsafe {
    //             kmapper.swap_into();
    //         }
    //         debug!("Kernel has finalized control of page tables.");
    //     });
    // }

    /* PARSE ACPI TABLES */
    // {
    //     crate::acpi::TABLES.call_once(|| {
    //         // let rsdp_address =
    //         //     RSDP_ADDRESS_REQUEST.get_response().expect("no response to RSDP address request").address();
    //         // // Safety: Bootloader guarantees the provided RDSP address is valid.
    //         // let acpi_tables = unsafe { acpi::AcpiTables::from_rsdp(crate::acpi::AcpiHandler, rsdp_address) }
    //         //     .expect("failed to parse ACPI tables");

    //         // spin::Mutex::new(acpi_tables)

    //         todo!()
    //     });
    // }

    // crate::mem::io::pci::init_devices().unwrap();

    // load_drivers();

    crate::cpu::start_mp(&MP_REQUEST);

    // Drop into a finalizing function to lose all references
    // to Limine bootloader requests/responses (they will be
    // deallocated during reclamation of bootloader memory).
    // finalize_init(memory_map)
    todo!()
}

/// Finalizes the kernel init process. After entering this function, all bootloader
/// reclaimable memory will be freed, and bootloader info/data will be inaccessible.
fn finalize_init(memory_map: &[&memory_map::Entry]) -> ! {
    debug!("Reclaiming bootloader memory...");

    memory_map
        .iter()
        .filter(|entry| entry.entry_type == limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE)
        .flat_map(|entry| {
            let entry_start = usize::try_from(entry.base).unwrap();
            let entry_end = usize::try_from(entry.base + entry.length).unwrap();

            (entry_start..entry_end).step_by(libsys::page_size())
        })
        .map(|address| Address::<Frame>::new(address).unwrap())
        .for_each(|frame| PhysicalMemoryManager::free_frame(frame).unwrap());

    debug!("Bootloader memory reclaimed.");

    // Safety: We've reached the end of the kernel init phase.
    unsafe { crate::cpu::run() }
}

// fn load_drivers() {
//     use crate::task::{AddressSpace, Priority, Task};
//     use elf::endian::AnyEndian;

//     #[limine::limine_tag]
//     static LIMINE_MODULES: limine::ModuleRequest = limine::ModuleRequest::new(crate::init::boot::LIMINE_REV);

//     debug!("Unpacking kernel drivers...");

//     let Some(modules) = LIMINE_MODULES.get_response() else {
//         warn!("Bootloader provided no modules; skipping driver loading.");
//         return;
//     };

//     let modules = modules.modules();
//     trace!("Found modules: {:X?}", modules);

//     let Some(drivers_module) = modules.iter().find(|module| module.path().ends_with("drivers")) else {
//         panic!("no drivers module found")
//     };

//     let archive = tar_no_std::TarArchiveRef::new(drivers_module.data());
//     archive
//         .entries()
//         .filter_map(|entry| {
//             debug!("Attempting to parse driver blob: {}", entry.filename());

//             match elf::ElfBytes::<AnyEndian>::minimal_parse(entry.data()) {
//                 Ok(elf) => Some((entry, elf)),
//                 Err(err) => {
//                     error!("Failed to parse driver blob into ELF: {:?}", err);
//                     None
//                 }
//             }
//         })
//         .for_each(|(entry, elf)| {
//             // Get and copy the ELF segments into a small box.
//             let Some(segments_copy) = elf.segments().map(|segments| segments.into_iter().collect()) else {
//                 error!("ELF has no segments.");
//                 return;
//             };

//             // Safety: In-place transmutation of initialized bytes for the purpose of copying safely.
//             // let (_, archive_data, _) = unsafe { entry.data().align_to::<MaybeUninit<u8>>() };
//             trace!("Allocating ELF data into memory...");
//             let elf_data = alloc::boxed::Box::from(entry.data());
//             trace!("ELF data allocated into memory.");

//             let Ok((Some(shdrs), Some(_))) = elf.section_headers_with_strtab() else {
//                 panic!("Error retrieving ELF relocation metadata.")
//             };

//             let load_offset = crate::task::MIN_LOAD_OFFSET;

//             trace!("Processing relocations localized to fault page.");
//             let mut relas = alloc::vec::Vec::with_capacity(shdrs.len());

//             shdrs
//                 .iter()
//                 .filter(|shdr| shdr.sh_type == elf::abi::SHT_RELA)
//                 .flat_map(|shdr| elf.section_data_as_relas(&shdr).unwrap())
//                 .for_each(|rela| {
//                     use crate::task::ElfRela;

//                     match rela.r_type {
//                         elf::abi::R_X86_64_RELATIVE => relas.push(ElfRela {
//                             address: Address::new(usize::try_from(rela.r_offset).unwrap()).unwrap(),
//                             value: load_offset + usize::try_from(rela.r_addend).unwrap(),
//                         }),

//                         _ => unimplemented!(),
//                     }
//                 });

//             trace!("Finished processing relocations, pushing task.");

//             let task = Task::new(
//                 Priority::Normal,
//                 AddressSpace::new_userspace(),
//                 load_offset,
//                 elf.ehdr,
//                 segments_copy,
//                 relas,
//                 crate::task::ElfData::Memory(elf_data),
//             );

//             crate::task::PROCESSES.lock().push_back(task);
//         });
// }
