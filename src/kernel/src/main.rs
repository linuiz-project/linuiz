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
    let_chains,
    if_let_guard,
    ptr_as_uninit,
    strict_provenance_lints,
    box_vec_non_null
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
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]

extern crate alloc;

#[macro_use]
extern crate log;

#[macro_use]
extern crate thiserror;

#[macro_use]
extern crate zerocopy;

#[macro_use]
extern crate num_enum;

#[macro_use]
extern crate paste;

// mod acpi;
mod arch;
mod clock;
mod cpu;
mod error;
mod interrupts;
mod logging;
mod mem;
mod panic;
mod params;
mod rand;
mod task;
mod util;

#[macro_use]
extern crate bitflags;

use limine::{
    BaseRevision,
    mp::RequestFlags,
    request::{
        BootloaderInfoRequest, ExecutableAddressRequest, ExecutableCmdlineRequest,
        ExecutableFileRequest, HhdmRequest, MemoryMapRequest, MpRequest, RsdpRequest,
        StackSizeRequest,
    },
};

/// Specify the Limine revision to use.
#[doc(hidden)]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(3);

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
    static RSDP_ADDRESS_REQUEST: RsdpRequest = RsdpRequest::new();
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
    crate::panic::tracing::symbols::parse(&KERNEL_FILE_REQUEST);

    crate::mem::Hhdm::init(&HHDM_REQUEST);
    crate::mem::pmm::PhysicalMemoryManager::init(&MEMORY_MAP_REQUEST);
    crate::mem::init(
        &MEMORY_MAP_REQUEST,
        &KERNEL_FILE_REQUEST,
        &KERNEL_ADDRESS_REQUEST,
    );

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
