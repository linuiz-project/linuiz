#![no_std]
#![no_main]
#![feature(sync_unsafe_cell, naked_functions, asm_const, asm_sym)]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
extern "C" fn _start() -> ! {
    loop {
        unsafe {
            core::arch::asm!(
                "
                push rdi

                mov rdi, 0x10000000
                syscall

                pop rdi
                ",
                options(nostack, nomem)
            );

            loop {}
        }
    }
}
