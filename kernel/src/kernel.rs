#![no_std]
#![no_main]

mod io;

use core::panic::PanicInfo;
use io::vga_buffer;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // vga_buffer::safe_lock(|writer| {
    //     writer.write_string("TEST");
    // });

    loop {}
}
