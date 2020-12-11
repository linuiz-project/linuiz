#![no_std]
#![no_main]
#![feature(asm)]
#![feature(alloc_error_handler)]

mod drivers;
mod io;

use core::{alloc::Layout, panic::PanicInfo};
use efi_boot::{SystemTable, Runtime, Framebuffer};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[alloc_error_handler]
fn alloc_error(error: Layout) -> ! {
    loop {}
}

entrypoint!(kernel_main);
fn kernel_main(runtime_table: SystemTable<Runtime>, framebuffer: Framebuffer) -> i32 {
    loop {}
}

pub struct GDT {
    table: [u64; 8],
    free_index: usize,
}

pub struct GDTEntry {

}