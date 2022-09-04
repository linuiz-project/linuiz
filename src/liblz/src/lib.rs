#![no_std]

#[cfg(feature = "panic_handler")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

