#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;

use efi_boot::{entrypoint, Framebuffer};

entrypoint!(kernel_main);
extern "win64" fn kernel_main(_framebuffer: Option<Framebuffer>) -> i32 {
    if let Err(error) = gsai::logging::init() {
        panic!("{}", error);
    }

    info!("Successfully loaded into kernel, with logging enabled.");
    debug!("Initializing CPU structures.");

    init();

    // x86_64::instructions::interrupts::int3();

    loop {}

    0
}

fn init() {
    gsai::structures::gdt::init();
    debug!("Successfully initialized GDT.");
    gsai::structures::idt::init();
    debug!("Successfully initialized IDT.");
    gsai::structures::pic::init();
    debug!("Successfully initialized PIC.");

    x86_64::instructions::interrupts::enable();
    debug!("(WARN: interrupts are now enabled)");
}
