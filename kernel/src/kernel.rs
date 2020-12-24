#![no_std]
#![no_main]
#![feature(asm)]

use efi_boot::{entrypoint, Framebuffer};
use gsai::writeln;

entrypoint!(kernel_main);
extern "win64" fn kernel_main(_framebuffer: Option<Framebuffer>) -> i32 {
    writeln!("Successfully loaded into kernel.");
    writeln!("Initializing CPU structures.");

    writeln!("{}", core::mem::size_of::<gsai::Address>());
    loop {}
    init();

    loop {}
}

fn init() {
    gsai::structures::gdt::init();
    writeln!("Successfully initialized GDT.");
    loop {}
    gsai::structures::pic::init();
    writeln!("Successfully initialized PIC.");
    gsai::structures::idt::init();
    writeln!("Successfully initialized and configured IDT.");

    gsai::instructions::interrupts::enable();
    writeln!("(WARN: Interrupts are now enabled)");
}
