#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate gsai;

use efi_boot::{entrypoint, Framebuffer};

entrypoint!(kernel_main);
extern "win64" fn kernel_main(_framebuffer: Option<Framebuffer>) -> i32 {
    serialln!("Successfully loaded into kernel.");
    serialln!("Initializing CPU structures.");

    init();

    loop {}
}

fn init() {
    gsai::structures::gdt::init();
    serialln!("Successfully initialized GDT.");
    loop {}
    gsai::structures::pic::init();
    serialln!("Successfully initialized PIC.");
    gsai::structures::idt::init();
    serialln!("Successfully initialized and configured IDT.");

    gsai::instructions::interrupts::enable();
    serialln!("(WARN: Interrupts are now enabled)");
}
