#![no_std]
#![no_main]
#![feature(
    allocator_api,
    array_repeat,
    array_windows,
    box_vec_non_null,
    breakpoint,
    cfg_select,
    duration_constants,
    extern_types,
    generic_atomic,
    if_let_guard,
    iter_advance_by,
    iter_array_chunks,
    iter_next_chunk,
    maybe_uninit_array_assume_init,
    maybe_uninit_slice,
    maybe_uninit_write_slice,
    pointer_is_aligned_to,
    pointer_try_cast_aligned,
    ptr_as_ref_unchecked,
    ptr_as_uninit,
    range_into_bounds,
    slice_ptr_get,
    step_trait
)]

use limine::{
    BaseRevision,
    mp::RequestFlags,
    request::{
        BootloaderInfoRequest, ExecutableAddressRequest, ExecutableCmdlineRequest,
        ExecutableFileRequest, HhdmRequest, MemoryMapRequest, MpRequest, RsdpRequest,
        StackSizeRequest,
    },
};

mod acpi;
mod arch;
mod cpu;
mod interrupts;
mod logging;
mod mem;
mod panic;
mod params;
mod rand;
mod task;
mod time;
mod util;

#[cfg(debug_assertions)]
mod dev;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate log;

#[macro_use]
extern crate paste;

#[macro_use]
extern crate static_assertions;

#[macro_use]
extern crate thiserror;

unsafe extern "C" {
    pub type LinkerSymbol;
}

impl LinkerSymbol {
    pub fn as_usize(&'static self) -> usize {
        core::ptr::from_ref(self).addr()
    }
}

/// Specify the Limine revision to use.
#[doc(hidden)]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(4);

const KERNEL_STACK_SIZE: usize = {
    #[cfg(debug_assertions)]
    {
        0x40_0000
    }
    #[cfg(not(debug_assertions))]
    {
        0x10_0000
    }
};

/// Specify the exact stack size the kernel would like to use.
#[doc(hidden)]
#[allow(clippy::as_conversions)]
static STACK_SIZE_REQUEST: StackSizeRequest =
    StackSizeRequest::new().with_size(KERNEL_STACK_SIZE as u64);

#[doc(hidden)]
#[unsafe(no_mangle)]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    // All of the code within this function should be run ONLY ONCE. Writing the
    // code sequentially within one function easily ensures that will be the
    // case.

    // All limine feature requests (ensures they are not used after bootloader
    // memory is reclaimed)
    static BOOTLOADER_INFO_REQUEST: BootloaderInfoRequest = BootloaderInfoRequest::new();
    static KERNEL_FILE_REQUEST: ExecutableFileRequest = ExecutableFileRequest::new();
    static KERNEL_CMDLINE_REQUEST: ExecutableCmdlineRequest = ExecutableCmdlineRequest::new();
    static KERNEL_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();
    static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
    static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
    static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();
    static MP_REQUEST: MpRequest = MpRequest::new().with_flags(RequestFlags::X2APIC);

    // Enable logging first, so we can get feedback on the entire init process.
    crate::logging::KernelLogger::init();
    // The higher-half direct map is used by the local APIC, and so must be
    // initialized directly after the logger (so the logger can use the APIC
    // to determine the processor ID).
    crate::mem::HigherHalfDirectMap::init(&HHDM_REQUEST);

    #[cfg(target_arch = "x86_64")]
    crate::arch::x86_64::devices::local_apic::LocalApic::init();

    // Safety: Function is run only once for this processor.
    unsafe {
        crate::cpu::configure();
    }

    if STACK_SIZE_REQUEST.get_response().is_none() {
        warn!("Stack size request was not fulfilled.");
    }

    print_env_info(&BOOTLOADER_INFO_REQUEST, &MEMORY_MAP_REQUEST);

    let (kernel_physical_address, kernel_virtual_address) = KERNEL_ADDRESS_REQUEST
        .get_response()
        .map(|response| {
            (
                usize::try_from(response.physical_base()).unwrap(),
                usize::try_from(response.virtual_base()).unwrap(),
            )
        })
        .expect("bootloader did not provide a response to kernel address request");
    debug!("Kernel physical address: {kernel_physical_address:#X?}");
    debug!("Kernel virtual address: {kernel_virtual_address:#X?}");

    crate::params::KernelParameters::init(&KERNEL_CMDLINE_REQUEST);

    #[cfg(feature = "panic_traces")]
    if !crate::params::KernelParameters::drop_symbol_info() {
        crate::panic::tracing::symbols::KernelSymbols::init(&KERNEL_FILE_REQUEST);
    }

    crate::mem::pmm::PhysicalMemoryManager::init(&MEMORY_MAP_REQUEST);
    crate::mem::KernelMapper::init(
        &MEMORY_MAP_REQUEST,
        &KERNEL_FILE_REQUEST,
        &KERNEL_ADDRESS_REQUEST,
    );
    // Safety: Kernel mappings have just been set up.
    unsafe {
        crate::mem::KernelMapper::swap_into();
    }

    crate::time::Stopwatch::init(&RSDP_REQUEST);

    // Safety: We've reached the end of the kernel init phase.
    unsafe { crate::cpu::start(Some(&MP_REQUEST), Some(&MEMORY_MAP_REQUEST)) }
}

fn print_env_info(
    bootloader_info_request: &BootloaderInfoRequest,
    memory_map_request: &MemoryMapRequest,
) {
    if let Some(bootloader_info) = bootloader_info_request.get_response() {
        info!(
            "Bootloader: {} v{} (rev {})",
            bootloader_info.name(),
            bootloader_info.version(),
            bootloader_info.revision()
        );
    } else {
        info!("Bootloader: UNKNOWN");
    }

    #[cfg(target_arch = "x86_64")]
    {
        if let Some(hypervisor_info) = crate::arch::x86_64::cpuid::hypervisor_info() {
            info!("Hypervisor: {:?}", hypervisor_info.identify());
        }

        crate::arch::x86_64::cpuid::print_info();
    }

    let memory_map = memory_map_request
        .get_response()
        .expect("bootloader did not provide a response to the memory map request")
        .entries();

    report_memory_map_entries(memory_map);
    report_total_usable_memory(memory_map);
}

fn limine_memory_map_entry_type_to_str(entry_type: limine::memory_map::EntryType) -> &'static str {
    match entry_type {
        limine::memory_map::EntryType::USABLE => "USABLE",
        limine::memory_map::EntryType::RESERVED => "RESERVED",
        limine::memory_map::EntryType::EXECUTABLE_AND_MODULES => "EXECUTABLE_AND_MODULES",
        limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE => "BOOTLOADER_RECLAIMABLE",
        limine::memory_map::EntryType::ACPI_RECLAIMABLE => "ACPI_RECLAIMABLE",
        limine::memory_map::EntryType::ACPI_NVS => "ACPI_NVS",
        limine::memory_map::EntryType::FRAMEBUFFER => "FRAMEBUFFER",
        limine::memory_map::EntryType::BAD_MEMORY => "BAD_MEMORY",

        _ => "!! UNKOWN !!",
    }
}

fn report_memory_map_entries(memory_map: &[&limine::memory_map::Entry]) {
    memory_map.iter().for_each(|entry| {
        let entry_start = entry.base;
        let entry_end = entry_start + entry.length;
        debug!(
            "Memory Map: {:#X?}  {}",
            entry_start..entry_end,
            limine_memory_map_entry_type_to_str(entry.entry_type)
        );
    });
}

fn report_total_usable_memory(memory_map: &[&limine::memory_map::Entry]) {
    let total_usable_memory = memory_map
        .iter()
        .fold(0u64, |mut total_usable_memory, entry| {
            if matches!(
                entry.entry_type,
                limine::memory_map::EntryType::USABLE
                    | limine::memory_map::EntryType::EXECUTABLE_AND_MODULES
                    | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
                    | limine::memory_map::EntryType::ACPI_RECLAIMABLE
            ) {
                total_usable_memory += entry.length;
            }

            total_usable_memory
        });

    debug!(
        "Detected system memory: {}MB",
        total_usable_memory / 1_000_000
    );
}

// fn load_drivers() {
//     use crate::task::{AddressSpace, Priority, Task};
//     use elf::endian::AnyEndian;

//     #[limine::limine_tag]
//     static LIMINE_MODULES: limine::ModuleRequest =
// limine::ModuleRequest::new(crate::init::boot::LIMINE_REV);

//     debug!("Unpacking kernel drivers...");

//     let Some(modules) = LIMINE_MODULES.get_response() else {
//         warn!("Bootloader provided no modules; skipping driver loading.");
//         return;
//     };

//     let modules = modules.modules();
//     trace!("Found modules: {:X?}", modules);

//     let Some(drivers_module) = modules.iter().find(|module|
// module.path().ends_with("drivers")) else {         panic!("no drivers module
// found")     };

//     let archive = tar_no_std::TarArchiveRef::new(drivers_module.data());
//     archive
//         .entries()
//         .filter_map(|entry| {
//             debug!("Attempting to parse driver blob: {}", entry.filename());

//             match elf::ElfBytes::<AnyEndian>::minimal_parse(entry.data()) {
//                 Ok(elf) => Some((entry, elf)),
//                 Err(err) => {
//                     error!("Failed to parse driver blob into ELF: {:?}",
// err);                     None
//                 }
//             }
//         })
//         .for_each(|(entry, elf)| {
//             // Get and copy the ELF segments into a small box.
//             let Some(segments_copy) = elf.segments().map(|segments|
// segments.into_iter().collect()) else {                 error!("ELF has no
// segments.");                 return;
//             };

//             // Safety: In-place transmutation of initialized bytes for the
// purpose of copying safely.             // let (_, archive_data, _) = unsafe {
// entry.data().align_to::<MaybeUninit<u8>>() };             trace!("Allocating
// ELF data into memory...");             let elf_data =
// alloc::boxed::Box::from(entry.data());             trace!("ELF data allocated
// into memory.");

//             let Ok((Some(shdrs), Some(_))) =
// elf.section_headers_with_strtab() else {                 panic!("Error
// retrieving ELF relocation metadata.")             };

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
//                             address:
// Address::new(usize::try_from(rela.r_offset).unwrap()).unwrap(),
// value: load_offset + usize::try_from(rela.r_addend).unwrap(),
// }),

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

#[macro_export]
macro_rules! singleton {
    (
        $(#[$struct_attrs:meta])*
        $struct_scope:vis struct $struct_name:ident {
            $(
                $(#[$field_attrs:meta])*
                $field_scope:vis $field_name:ident: $field_ty:ty
            ),*
        }

        $(#[$init_attrs:meta])*
        fn init($($arg_name:ident: $arg_ty:ty),*) -> Self
            $init:block
    ) => {
        paste! {
            #[allow(non_upper_case_globals)]
            static [< STATIC_ $struct_name >]: spin::Once<$struct_name> = spin::Once::new();

            $(#[$struct_attrs])*
            $struct_scope struct $struct_name {
                $(
                    $(#[$field_attrs])*
                    $field_scope $field_name: $field_ty
                ),*
            }

            impl $struct_name {
                $(#[$init_attrs])*
                pub fn init($($arg_name: $arg_ty),*) {
                    let init_fn = || $init;

                    [< STATIC_ $struct_name >].call_once(init_fn);
                }

                /// Gets the single instance of [`Self`], or causes a panic if it's uninitialized.
                fn get_static() -> &'static Self {
                    [< STATIC_ $struct_name >]
                        .get()
                        .expect(
                            concat!("static `", stringify!($struct_name), "` has not yet been initialized")
                        )
                }

                /// Whether the singleton has been initialized.
                pub fn is_initialized() -> bool {
                    [< STATIC_ $struct_name >].get().is_some()
                }
            }
        }
    };
}
