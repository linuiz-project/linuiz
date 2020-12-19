#![no_std]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

mod privilege_level;

pub mod drivers;
pub mod instructions;
pub mod io;
pub mod structures;
pub use privilege_level::PrivilegeLevel;

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

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Address {
    Physical(usize),
    Virtual(usize),
}
