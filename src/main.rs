#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![feature(
    allocator_api,
    array_repeat,
    array_windows,
    ascii_char,
    breakpoint,
    cfg_select,
    const_from,
    const_option_ops,
    const_trait_impl,
    const_try,
    core_intrinsics,
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
    step_trait,
    unsafe_cell_access
)]
#![forbid(clippy::duplicated_attributes, clippy::inline_asm_x86_att_syntax)]
#![deny(
    clippy::debug_assert_with_mut_call,
    clippy::float_arithmetic,
    stable_features
)]
#![warn(
    clippy::as_conversions,
    clippy::pedantic,
    clippy::undocumented_unsafe_blocks,
    clippy::semicolon_inside_block,
    clippy::semicolon_if_nothing_returned,
    clippy::unreadable_literal,
    clippy::perf,
    unsafe_op_in_unsafe_fn,
    unused_crate_dependencies
)]
#![cfg_attr(debug_assertions, allow(clippy::todo))]
#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::cargo_common_metadata,
    clippy::enum_glob_use,
    clippy::inline_always,
    clippy::items_after_statements,
    clippy::must_use_candidate,
    clippy::missing_const_for_fn,
    clippy::needless_for_each,
    clippy::if_not_else,
    clippy::similar_names,
    dead_code,
    mismatched_lifetime_syntaxes,
    internal_features
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
mod scheduler;
mod time;
mod util;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate log;

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

    crate::acpi::init_tables(&RSDP_REQUEST);

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
