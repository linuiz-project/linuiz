#![no_std]
#![no_main]
#![feature(
    iter_advance_by,                        // #77404 <https://github.com/rust-lang/rust/issues/77404>
    iter_array_chunks,                      // #100450 <https://github.com/rust-lang/rust/issues/100450>
    iter_next_chunk,                        // #98326 <https://github.com/rust-lang/rust/issues/98326>
    array_windows,                          // #75027 <https://github.com/rust-lang/rust/issues/75027>
    maybe_uninit_slice,                     // #63569 <https://github.com/rust-lang/rust/issues/63569>
    maybe_uninit_write_slice,               // #79995 <https://github.com/rust-lang/rust/issues/79995>
    iterator_try_reduce,                    // #87053 <https://github.com/rust-lang/rust/issues/87053>
    map_try_insert,                         // #82766 <https://github.com/rust-lang/rust/issues/82766>
    try_trait_v2,                           // #84277 <https://github.com/rust-lang/rust/issues/84277>
    step_trait,                             // #42168 <https://github.com/rust-lang/rust/issues/42168>
    generic_arg_infer,                      // #85077 <https://github.com/rust-lang/rust/issues/85077>
    exclusive_wrapper,                      // #98407 <https://github.com/rust-lang/rust/issues/98407>
    nonnull_provenance,                     // #135243 <https://github.com/rust-lang/rust/issues/135243>
    sync_unsafe_cell,
    slice_ptr_get,
    let_chains,
    if_let_guard,
    exact_size_is_empty,
    fn_align,
    ptr_as_uninit,
    ptr_metadata,
    btreemap_alloc,
    const_trait_impl,
)]
#![forbid(clippy::inline_asm_x86_att_syntax)]
#![deny(clippy::debug_assert_with_mut_call, clippy::float_arithmetic)]
#![warn(
    clippy::cargo,
    clippy::pedantic,
    clippy::cast_lossless,
    clippy::missing_const_for_fn,
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
    // // While ideally this is warned against, the number of situations in which pointer alignment up-casting
    // // is acceptable seem to far outweigh the circumstances within the kernel where it is inappropriate.
    // clippy::cast_ptr_alignment,
    dead_code
)]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]

extern crate alloc;

#[macro_use]
extern crate log;

#[macro_use]
extern crate thiserror;

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

/// ## Safety
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
