#![no_std]

mod io;

use core::panic::PanicInfo;

use io::vga_buffer::{ColorCode, ScreenBuffer, ScreenChar, VGAColor};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let buffer = 0xB8000 as *mut u8;

    unsafe {
        *buffer.offset(0) = b'P';
    }

    loop {}
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    // println!("X");

    let buffer = 0xB8000 as *mut u8;
    unsafe {
        *buffer.offset(0) = b'7';
    }

    loop {  }    
}
