#![no_std]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

pub mod boot;
pub mod drivers;

use core::{alloc::Layout, panic::PanicInfo};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[alloc_error_handler]
fn alloc_error(_error: Layout) -> ! {
    loop {}
}
