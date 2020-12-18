#![no_std]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

pub mod boot;
pub mod drivers;
pub mod io;

use core::{alloc::Layout, panic::PanicInfo};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    write!("{}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error(error: Layout) -> ! {
    write!("{:?}", error);
    loop {}
}
