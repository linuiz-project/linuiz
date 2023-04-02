#![no_std]
#![no_main]
#![feature(
    error_in_core,                          // #103765 <https://github.com/rust-lang/rust/issues/103765>
    is_some_and,                            // #93050 <https://github.com/rust-lang/rust/issues/93050>
    result_flattening,                      // #70142 <https://github.com/rust-lang/rust/issues/70142>
    map_try_insert,                         // #82766 <https://github.com/rust-lang/rust/issues/82766>
    asm_const,
    naked_functions,
    sync_unsafe_cell,
    panic_info_message,
    allocator_api,
    pointer_is_aligned,
    slice_ptr_get,
    strict_provenance,
    core_intrinsics,
    alloc_error_handler,
    exclusive_range_pattern,
    raw_ref_op,
    let_chains,
    unchecked_math,
    if_let_guard,
    exact_size_is_empty,
    fn_align,
    ptr_as_uninit,
    ptr_metadata,
    control_flow_enum,
    btreemap_alloc,
    inline_const,
    const_option,
    const_option_ext,
    const_trait_impl,
    const_cmp
)]
#![forbid(clippy::inline_asm_x86_att_syntax, clippy::missing_const_for_fn)]
#![deny(clippy::semicolon_if_nothing_returned, clippy::debug_assert_with_mut_call, clippy::float_arithmetic)]
#![warn(clippy::cargo, clippy::pedantic, clippy::undocumented_unsafe_blocks)]
#![allow(
    clippy::cast_lossless,
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

#[cfg(target_pointer_width = "32")]
#[allow(non_camel_case_types)]
pub type psize = u32;
#[cfg(target_pointer_width = "64")]
#[allow(non_camel_case_types)]
pub type psize = u64;

mod acpi;
mod arch;
mod boot;
mod cpu;
mod exceptions;
mod init;
mod interrupts;
mod local_state;
mod logging;
mod memory;
mod panic;
mod proc;
mod rand;
mod time;

#[macro_export]
macro_rules! default_display_impl {
    ($name:ident) => {
        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Debug::fmt(self, f)
            }
        }
    };
}

#[macro_export]
macro_rules! err_result_type {
    ($name:ident) => {
        pub type Result<T> = core::result::Result<T, $name>;
    };
}
