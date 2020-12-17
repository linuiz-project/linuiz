#![no_std]
#![no_main]

use efi_boot::{entrypoint, Framebuffer};

entrypoint!(kernel_main);
extern "win64" fn kernel_main(framebuffer: Option<Framebuffer>) -> i32 {
    init();

    loop {}
}

fn init() {
    kernel::boot::gdt::init();
    kernel::boot::interrupts::load_idt();
    kernel::boot::pic::init();
    x86_64::instructions::interrupts::enable();
}
