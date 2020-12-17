#![no_std]
#![no_main]
#![feature(asm)]

use efi_boot::{entrypoint, Framebuffer};
use gsai::writeln;

entrypoint!(kernel_main);
extern "win64" fn kernel_main(framebuffer: Option<Framebuffer>) -> i32 {
    writeln!("Successfully loaded into kernel.");
    writeln!("Initializing CPU structures.");

    init();

    loop {}
}

fn init() {
    gsai::boot::gdt::init();
    writeln!("Successfully initialized GDT.");
    gsai::boot::pic::init();
    writeln!("Successfully initialized PIC.");
    gsai::boot::interrupts::load_idt();
    writeln!("Successfully initialized and configured IDT.");

    x86_64::instructions::interrupts::enable();
    writeln!("(WARN: Interrupts are now enabled)");
}
