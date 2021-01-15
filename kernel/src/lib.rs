#![no_std]
#![feature(asm)]
#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate log;

mod bitarray;
pub mod drivers;
pub mod instructions;
pub mod io;
pub mod logging;
pub mod registers;
pub mod structures;
pub use bitarray::BitArray;

use core::{alloc::Layout, panic::PanicInfo};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial!("\n{}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error(error: Layout) -> ! {
    serial!("{:?}", error);
    loop {}
}
