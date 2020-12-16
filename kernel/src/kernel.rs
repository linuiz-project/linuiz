#![no_std]
#![no_main]

extern crate kernel;

use efi_boot::{entrypoint, Framebuffer};
use kernel::drivers::serial;

entrypoint!(kernel_main);
extern "win64" fn kernel_main(framebuffer: Option<Framebuffer>) -> i32 {
    Serial::safe_lock(|serial| {
        serial.data_port().write(b'X');
    });

    // let mut framebuffer_driver = FramebufferDriver::new(
    //     framebuffer.unwrap().pointer as *mut Color8i,
    //     0xB71B000 as *mut Color8i,
    //     framebuffer.unwrap().size,
    // );

    // framebuffer_driver.clear(Colors::LightBlue.into(), true);

    loop {}
}
