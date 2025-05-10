#![no_std]
#![no_main]
#![feature(
    result_flattening,                      // #70142 <https://github.com/rust-lang/rust/issues/70142>
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
    sync_unsafe_cell,
    allocator_api,
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
    clippy::semicolon_if_nothing_returned
)]
#![allow(
    clippy::enum_glob_use,
    clippy::inline_always,
    clippy::items_after_statements,
    clippy::must_use_candidate,
    clippy::unreadable_literal,
    clippy::wildcard_imports,
    // While ideally this is warned against, the number of situations in which pointer alignment up-casting
    // is acceptable seem to far outweigh the circumstances within the kernel where it is inappropriate.
    clippy::cast_ptr_alignment,
    dead_code
)]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]

extern crate alloc;

#[macro_use]
extern crate log;

mod acpi;
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

#[cfg(debug_assertions)]
static STACK_SIZE: u64 = 0x1000000;
#[cfg(not(debug_assertions))]
static STACK_SIZE: u64 = 0x4000;

/// ### Safety
///
/// This function should only ever be called by the bootloader.
#[no_mangle]
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
