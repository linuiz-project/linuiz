#![no_std]
#![no_main]
#![feature(
    iter_advance_by,
    iter_array_chunks,
    iter_next_chunk,
    array_windows,
    maybe_uninit_slice,
    maybe_uninit_write_slice,
    step_trait,
    breakpoint,
    extern_types,
    slice_ptr_get,
    if_let_guard,
    ptr_as_uninit,
    strict_provenance_lints,
    box_vec_non_null,
    allocator_api,
    duration_constants,
    array_ptr_get
)]
#![forbid(clippy::inline_asm_x86_att_syntax, fuzzy_provenance_casts)]
#![deny(
    clippy::debug_assert_with_mut_call,
    clippy::float_arithmetic,
    clippy::as_conversions,
    stable_features
)]
#![warn(
    clippy::cargo,
    clippy::pedantic,
    clippy::undocumented_unsafe_blocks,
    clippy::semicolon_inside_block,
    clippy::semicolon_if_nothing_returned,
    unsafe_op_in_unsafe_fn
)]
#![allow(
    clippy::cargo_common_metadata,
    clippy::enum_glob_use,
    clippy::inline_always,
    clippy::items_after_statements,
    clippy::must_use_candidate,
    clippy::unreadable_literal,
    clippy::wildcard_imports,
    clippy::upper_case_acronyms,
    clippy::missing_const_for_fn,
    clippy::needless_for_each,
    clippy::if_not_else,
    dead_code
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

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate log;

#[macro_use]
extern crate num_enum;

#[macro_use]
extern crate paste;

#[macro_use]
extern crate thiserror;

#[macro_use]
extern crate zerocopy;

unsafe extern "C" {
    pub type LinkerSymbol;
}

impl LinkerSymbol {
    pub fn as_usize(&'static self) -> usize {
        (&raw const self).addr()
    }
}

/// Specify the Limine revision to use.
#[doc(hidden)]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(4);

const KERNEL_STACK_SIZE: usize = {
    #[cfg(debug_assertions)]
    {
        0x1000000
    }
    #[cfg(not(debug_assertions))]
    {
        0x4000
    }
};

/// Specify the exact stack size the kernel would like to use.
#[doc(hidden)]
#[allow(clippy::as_conversions)]
static STACK_SIZE_REQUEST: StackSizeRequest =
    StackSizeRequest::new().with_size(KERNEL_STACK_SIZE as u64);

/// # Safety
///
/// This function should only ever be called by the bootloader.
#[doc(hidden)]
#[unsafe(no_mangle)]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    // This function is absolutely massive, and that's intentional. All of the code
    // within this function should be absolutely, definitely run ONLY ONCE. Writing
    // the code sequentially within one function easily ensures that will be the case.

    // All limine feature requests (ensures they are not used after bootloader memory is reclaimed)
    static BOOTLOADER_INFO_REQUEST: BootloaderInfoRequest = BootloaderInfoRequest::new();
    static KERNEL_FILE_REQUEST: ExecutableFileRequest = ExecutableFileRequest::new();
    static KERNEL_CMDLINE_REQUEST: ExecutableCmdlineRequest = ExecutableCmdlineRequest::new();
    static KERNEL_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();
    static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
    static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
    static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();
    static MP_REQUEST: MpRequest = MpRequest::new().with_flags(RequestFlags::X2APIC);

    // Enable logging first, so we can get feedback on the entire init process.
    crate::logging::Logger::init();

    // Safety: Function is run only once for this hardware thread.
    unsafe {
        #[cfg(target_arch = "x86_64")]
        crate::arch::x86_64::configure_hwthread();
    }

    print_boot_info(&BOOTLOADER_INFO_REQUEST);

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

    crate::params::parse(&KERNEL_CMDLINE_REQUEST);

    #[cfg(feature = "panic_traces")]
    if crate::params::keep_symbol_info() {
        crate::panic::tracing::symbols::Symbols::init(&KERNEL_FILE_REQUEST);
    }

    crate::mem::HigherHalfDirectMap::init(&HHDM_REQUEST);
    crate::mem::pmm::PhysicalMemoryManager::init(&MEMORY_MAP_REQUEST);
    crate::mem::init(
        &MEMORY_MAP_REQUEST,
        &KERNEL_FILE_REQUEST,
        &KERNEL_ADDRESS_REQUEST,
    );

    crate::time::Stopwatch::init(&RSDP_REQUEST);
    trace!("System stopwatch initialized.");

    // Safety: We've reached the end of the kernel init phase.
    unsafe { crate::cpu::synchronize(Some((&MP_REQUEST, &MEMORY_MAP_REQUEST))) }
}

fn print_boot_info(bootloader_info_request: &BootloaderInfoRequest) {
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

#[macro_export]
macro_rules! singleton {
    (
        $(#[$struct_attrs:meta])*
        $struct_scope:vis $struct_name:ident {
            $(
                $(#[$field_attrs:meta])*
                $scope:vis $field_name:ident: $field_ty:ty,
            )*
        }

        $(#[$init_attrs:meta])*
        fn init($($arg_name:ident: $arg_ty:ty),*)
            $init:block
    ) => {
        paste! {
            #[allow(non_upper_case_globals)]
            static [< STATIC_ $struct_name >]: spin::Once<$struct_name> = spin::Once::new();

            $(#[$struct_attrs])*
            $struct_scope struct $struct_name {
                $(
                    $(#[$field_attrs])*
                    $scope $field_name: $field_ty
                ),*
            }

            impl $struct_name {
                $(#[$init_attrs])*
                pub fn init($($arg_name: $arg_ty)*) {
                    [< STATIC_ $struct_name >].call_once(||{
                        trace!(concat!("Initializing `", stringify!($struct_name), "`..."));

                        let init = $init;

                        debug!(concat!("Static `", stringify!($struct_name), "` initialized."));

                        init
                    });
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
