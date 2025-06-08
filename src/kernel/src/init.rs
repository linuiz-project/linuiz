use libsys::{Address, Frame};
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
    static KERNEL_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();
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
    crate::mem::init(
        &MEMORY_MAP_REQUEST,
        &KERNEL_FILE_REQUEST,
        &KERNEL_ADDRESS_REQUEST,
    );

    crate::arch::x86_64::instructions::breakpoint();

    // /* PARSE ACPI TABLES */
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
