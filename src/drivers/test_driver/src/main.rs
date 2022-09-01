#![no_std]
#![no_main]
#![feature(start)]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[start]
extern "sysv64" fn _main() {
    loop {}
}
