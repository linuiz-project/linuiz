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
    box_vec_non_null,
    generic_const_exprs
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
extern crate strum;

// mod acpi;
mod arch;
mod cpu;
mod error;
mod init;
mod interrupts;
mod logging;
mod mem;
mod panic;
mod params;
mod rand;
mod task;
mod time;
mod util;

#[macro_use]
extern crate bitflags;

/// Specify the Limine revision to use.
#[doc(hidden)]
static BASE_REVISION: limine::BaseRevision = limine::BaseRevision::with_revision(0);

/// Specify the exact stack size the kernel would like to use.
#[doc(hidden)]
static STACK_SIZE_REQUEST: limine::request::StackSizeRequest =
    limine::request::StackSizeRequest::new().with_size({
        #[cfg(debug_assertions)]
        {
            0x1000000
        }
        #[cfg(not(debug_assertions))]
        {
            0x4000
        }
    });

/// # Safety
///
/// This function should only ever be called by the bootloader.
#[unsafe(no_mangle)]
#[doc(hidden)]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    // Safety: We've just entered the kernel, so no state can be disrupted.
    unsafe {
        core::arch::asm!(
            "
            xor rbp, rbp

            call {}
            ",
            sym init::init,
            options(noreturn)
        )
    }
}
