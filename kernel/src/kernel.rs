#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;

use efi_boot::{entrypoint, Framebuffer};
use gsai::logging::LOGGER;

entrypoint!(kernel_main);
extern "win64" fn kernel_main(_framebuffer: Option<Framebuffer>) -> i32 {
    if let Err(error) = unsafe { gsai::logging::init() } {
        panic!("{}", error);
    }

    info!("Successfully loaded into kernel.");
    debug!("Initializing CPU structures.");
    loop {}

    init();

    0
}

fn init() {
    gsai::structures::gdt::init();
    debug!("Successfully initialized GDT.");
    gsai::structures::pic::init();
    debug!("Successfully initialized PIC.");
    gsai::structures::idt::init();
    debug!("Successfully initialized and configured IDT.");

    gsai::instructions::interrupts::enable();
    debug!("(WARN: Interrupts are now enabled)");
}
