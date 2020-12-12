#![no_std]
#![no_main]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate pic8259_simple;

mod drivers;
mod gdt;
mod interrupts;
mod io;
mod pic;

use core::{alloc::Layout, panic::PanicInfo};
use drivers::serial;
use efi_boot::{entrypoint, Framebuffer};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[alloc_error_handler]
fn alloc_error(_error: Layout) -> ! {
    loop {}
}

entrypoint!(kernel_main);
extern "C" fn kernel_main(framebuffer: *const Framebuffer) -> i32 {
    if (framebuffer == 0x100 as *const Framebuffer) {
        loop {}
    }

    unsafe {
        asm!(
            "mov r8, {}",
            "mov rdx, [0xFFFFFFFFF]",
             in(reg) framebuffer
        );
    }

    init();

    serial::safe_lock(|serial| {
        serial
            .data_port()
            .write_buffer(&mut [b'H', b'E', b'L', b'L', b'O']);

        // loop {
        //     serial.data_port().write(b'X');
        // }
    });

    1234
}

fn init() {
    gdt::init();
    interrupts::load_idt();
    pic::init();
    x86_64::instructions::interrupts::enable();
}
