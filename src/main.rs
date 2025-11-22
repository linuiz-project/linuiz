#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![feature(
    allocator_api,
    array_windows,
    ascii_char,
    ascii_char_variants,
    breakpoint,
    cfg_select,
    const_convert,
    const_option_ops,
    const_trait_impl,
    const_try,
    core_intrinsics,
    duration_constants,
    exact_size_is_empty,
    extern_types,
    generic_atomic,
    if_let_guard,
    iter_advance_by,
    iter_array_chunks,
    iter_next_chunk,
    maybe_uninit_array_assume_init,
    maybe_uninit_slice,
    maybe_uninit_write_slice,
    nonzero_ops,
    pointer_is_aligned_to,
    pointer_try_cast_aligned,
    ptr_as_ref_unchecked,
    ptr_as_uninit,
    range_into_bounds,
    slice_ptr_get,
    step_trait,
    unchecked_shifts,
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

const KERNEL_STACK_SIZE: u64 = {
    cfg_select! {
        debug_assertions => { 0x8_0000 }
        not(debug_assertions) => { 0x1_0000 }
    }
};

#[doc(hidden)]
static BASE_REVISION: limine::BaseRevision = limine::BaseRevision::with_revision(4);

#[doc(hidden)]
#[allow(clippy::as_conversions)]
static STACK_SIZE_REQUEST: limine::request::StackSizeRequest =
    limine::request::StackSizeRequest::new().with_size(KERNEL_STACK_SIZE);

/// Clear the frame pointer so that on a kernel panic we don't trace anything
/// prior to this function.
#[macro_export]
macro_rules! naked_asm_clear_frame_pointer_and_call_fn {
    ($call_fn:ident) => {
        cfg_select! {
            target_arch = "x86_64" => {
                core::arch::naked_asm!(
                    "
                    xor rbp, rbp
                    call {}
                    ",
                    sym $call_fn
                )
            }

            _ => { unimplemented!() }
        }
    };
}

#[doc(hidden)]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn _entry() -> ! {
    naked_asm_clear_frame_pointer_and_call_fn!(main)
}

#[doc(hidden)]
extern "C" fn main() -> ! {
    // All of the code within this function should be run ONLY ONCE. Writing the
    // code sequentially within one function easily ensures that will be the
    // case.

    // All limine feature requests (ensures they are not used after bootloader
    // memory is reclaimed)
    static BOOTLOADER_INFO_REQUEST: limine::request::BootloaderInfoRequest =
        limine::request::BootloaderInfoRequest::new();
    static KERNEL_FILE_REQUEST: limine::request::ExecutableFileRequest =
        limine::request::ExecutableFileRequest::new();
    static KERNEL_CMDLINE_REQUEST: limine::request::ExecutableCmdlineRequest =
        limine::request::ExecutableCmdlineRequest::new();
    static KERNEL_ADDRESS_REQUEST: limine::request::ExecutableAddressRequest =
        limine::request::ExecutableAddressRequest::new();
    static HHDM_REQUEST: limine::request::HhdmRequest = limine::request::HhdmRequest::new();
    static MEMORY_MAP_REQUEST: limine::request::MemoryMapRequest =
        limine::request::MemoryMapRequest::new();
    static RSDP_REQUEST: limine::request::RsdpRequest = limine::request::RsdpRequest::new();
    static MP_REQUEST: limine::request::MpRequest =
        limine::request::MpRequest::new().with_flags(limine::mp::RequestFlags::X2APIC);

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

    // Safety: `MEMORY_MAP_REQUEST` has not been allocated from since entry.
    unsafe {
        crate::mem::pmm::PhysicalMemoryManager::init(&MEMORY_MAP_REQUEST);
    }

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
    unsafe { init_processor(Some(&MP_REQUEST), Some(&MEMORY_MAP_REQUEST)) }
}

fn print_env_info(
    bootloader_info_request: &limine::request::BootloaderInfoRequest,
    memory_map_request: &limine::request::MemoryMapRequest,
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

    memory_map.iter().for_each(|entry| {
        let entry_start = entry.base;
        let entry_end = entry_start + entry.length;
        debug!(
            "Memory Map: {:#X?}  {}",
            entry_start..entry_end,
            limine_memory_map_entry_type_to_str(entry.entry_type)
        );
    });

    let total_usable_memory = memory_map
        .iter()
        .fold(0u64, |mut total_usable_memory, entry| {
            match entry.entry_type {
                limine::memory_map::EntryType::USABLE
                | limine::memory_map::EntryType::EXECUTABLE_AND_MODULES
                | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
                | limine::memory_map::EntryType::ACPI_RECLAIMABLE => {
                    total_usable_memory += entry.length;
                }

                _ => {}
            }

            total_usable_memory
        });

    debug!(
        "Detected system memory: {}MB",
        total_usable_memory / 1_000_000
    );
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

/// Enters core into the scheduler loop, exiting the kernel's boot phase.
///
/// # Safety
///
/// - Function should only be run once at the end of the kernel boot phase.
#[allow(clippy::too_many_lines)]
pub unsafe fn init_processor(
    mp_request: Option<&limine::request::MpRequest>,
    _memory_map_request: Option<&limine::request::MemoryMapRequest>,
) -> ! {
    use crate::{cpu::local_state::LocalState, scheduler::Scheduler};

    /// Iterates the entries in the multiprocessing request, configuring and
    /// subsequently synchronizing the other processors in the system.
    ///
    /// # Returns
    ///
    /// - If request was satisfied, `Some` of the count of non-bootstrap
    ///   processor in the system.
    /// - If request was not satisfied, `None`.
    pub fn begin_multiprocessing(mp_request: &limine::request::MpRequest) -> Option<usize> {
        let Some(response) = mp_request.get_response() else {
            warn!("Bootloader did not provide response to multiprocessing request.");
            return None;
        };

        debug!("Detecting and starting additional cores.");

        let mp_entry = {
            if crate::params::KernelParameters::use_multiprocessing() {
                #[unsafe(naked)]
                extern "C" fn _mp_entry(_: &limine::mp::Cpu) -> ! {
                    extern "C" fn _mp_main(_: &limine::mp::Cpu) -> ! {
                        // Safety: Function is run only once for this processor.
                        unsafe {
                            crate::cpu::configure();
                        }

                        // Safety: All currently referenced memory should also be
                        //         mapped in the kernel page tables.
                        unsafe {
                            crate::mem::KernelMapper::swap_into();
                        }

                        // Safety: processor still in init phase.
                        unsafe { init_processor(None, None) }
                    }

                    crate::naked_asm_clear_frame_pointer_and_call_fn!(_mp_main)
                }

                _mp_entry
            } else {
                #[unsafe(naked)]
                extern "C" fn _idle_entry(_: &limine::mp::Cpu) -> ! {
                    extern "C" fn _idle_main(_: &limine::mp::Cpu) -> ! {
                        crate::cpu::halt_and_catch_fire()
                    }

                    crate::naked_asm_clear_frame_pointer_and_call_fn!(_idle_main)
                }

                _idle_entry
            }
        };

        response
            .cpus()
            .iter()
            .filter(|cpu| cpu.lapic_id != response.bsp_lapic_id())
            .for_each(|cpu| {
                trace!("Starting processor: ID#{} LAPIC#{}", cpu.id, cpu.lapic_id);

                cpu.goto_address.write(mp_entry);
            });

        Some(response.cpus().len())
    }

    mp_request
        .and_then(begin_multiprocessing)
        .inspect(|processor_count| {
            trace!("Detected {processor_count} processors.");
        });

    LocalState::init();

    debug!("Preparing for task scheduling...");
    LocalState::with_scheduler(Scheduler::enable);

    trace!("Enabling interrupts...");
    crate::interrupts::enable();

    LocalState::with_timer(|timer| {
        trace!("Enabling local timer...");
        timer.enable();
        trace!("Setting preemption wait...");
        timer.set_preemption_wait();
    });

    trace!("Waiting for preemption...");
    // Wait loop to ensure the core can jump into the scheduler upon timer fire.
    crate::interrupts::wait_indefinite()
}
