#![no_std]
#![feature(
    strict_provenance,          // #95228 <https://github.com/rust-lang/rust/issues/95228>
    const_option
)]

mod address;
pub use address::*;

mod constants;
pub use constants::*;
