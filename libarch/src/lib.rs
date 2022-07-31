#![no_std]
#![feature(const_mut_refs)]

mod addr;

pub use addr::*;
pub mod cpu;
pub mod instructions;
pub mod io;
pub mod registers;

// TODO remove `control` registers from `libkernel`
// TODO remove `addr` from `libkernel`
