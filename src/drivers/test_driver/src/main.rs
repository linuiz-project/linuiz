#![no_std]
#![feature(start)]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[start]
fn main(_arg_count: isize, _args: *const *const u8) -> isize {
    let mut sum = 0;
    for i in 0..10000 {
        sum += i;
    }

    sum + _arg_count
}
