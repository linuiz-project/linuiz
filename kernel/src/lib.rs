#![no_std]
#![feature(asm)]
#![feature(const_fn)]
#![feature(once_cell)]
#![feature(abi_efiapi)]
#![feature(const_mut_refs)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate log;

mod bitarray;
pub mod drivers;
pub mod instructions;
pub mod io;
pub mod linker_statics;
pub mod logging;
pub mod memory;
pub mod registers;
pub mod structures;
pub use bitarray::BitArray;

use core::{alloc::Layout, panic::PanicInfo};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serialln!("\n{}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error(error: Layout) -> ! {
    serial!("{:#?}", error);
    loop {}
}
