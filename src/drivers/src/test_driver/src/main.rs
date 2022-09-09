#![no_std]
#![no_main]
#![feature(sync_unsafe_cell, naked_functions, asm_const, asm_sym, cstr_from_bytes_until_nul)]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
extern "C" fn _start() -> ! {
    unsafe {
        let log_message = core::ffi::CStr::from_bytes_until_nul(b"process logging test\0").unwrap();

        core::arch::asm!(
            "syscall",
            in("rdi") 0x100,
            in("rsi") log::Level::Info as usize,
            in("rdx")  log_message.as_ptr(),
            options(nostack, nomem)
        );
    }

    loop {}
}
