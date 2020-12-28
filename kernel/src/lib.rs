#![no_std]
#![feature(asm)]
#![feature(const_fn)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate log;

pub mod drivers;
pub mod instructions;
pub mod io;
pub mod logging;
pub mod structures;

use core::{alloc::Layout, panic::PanicInfo};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial!("{}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error(error: Layout) -> ! {
    serial!("{:?}", error);
    loop {}
}
