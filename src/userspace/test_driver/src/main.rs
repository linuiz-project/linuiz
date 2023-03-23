#![no_std]
#![no_main]
#![feature(sync_unsafe_cell, naked_functions, asm_const)]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
extern "C" fn _start() -> ! {
   loop{}
   
    let log_message = core::ffi::CStr::from_bytes_until_nul(b"process logging test\0").unwrap();

    for _ in 0..10 {
        unsafe {
            core::arch::asm!(
                "
                push rax
                push rcx
                push r8
                push r9
                push r10
                push r11

                syscall

                pop r11
                pop r10
                pop r9
                pop r9
                pop rcx
                pop rax
                ",
                inout("rdi") 0x100 => _,
                inout("rsi") log::Level::Info as usize => _,
                inout("rdx")  log_message.as_ptr() => _,
                options(nostack, nomem)
            );
        }
    }

    loop {}
}
