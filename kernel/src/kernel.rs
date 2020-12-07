#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    unsafe {
        let mut buffer = 0xb8000 as *mut u8;
        buffer.offset(0) = b'X'
    };

    loop {}
}
