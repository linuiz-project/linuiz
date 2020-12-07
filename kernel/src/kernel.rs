#![no_std]
#![no_main]
#![feature(lang_items, start)]

mod io;

use core::panic::PanicInfo;

use io::vga_buffer;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
fn kernel_main() {
    // vga_buffer::safe_lock(|writer| {
    //     writer.write_string("TEST");
    // });
    
    loop {}
}
