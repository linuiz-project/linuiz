#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;

use efi_boot::{entrypoint, FramebufferPointer};

entrypoint!(kernel_main);
extern "win64" fn kernel_main(framebuffer_ptr: Option<FramebufferPointer>) -> i32 {
    if let Err(error) =
        gsai::logging::init(gsai::logging::LoggingModes::SERIAL, log::LevelFilter::Debug)
    {
        panic!("{}", error);
    }

    info!("Successfully loaded into kernel, with logging enabled.");

    debug!("Configuring CPU state.");
    unsafe { init_cpu_state() };
    debug!("Initializing memory structures.");
    init_structures();

    loop {}

    0
}

unsafe fn init_cpu_state() {
    gsai::instructions::init_segment_registers(0x0);
    debug!("Zeroed segment registers.");
}

fn init_structures() {
    gsai::structures::gdt::init();
    debug!("Successfully initialized GDT.");
    gsai::structures::idt::init();
    debug!("Successfully initialized IDT.");
    gsai::structures::pic::init();
    debug!("Successfully initialized PIC.");

    x86_64::instructions::interrupts::enable();
    debug!("(WARN: interrupts are now enabled)");
}
