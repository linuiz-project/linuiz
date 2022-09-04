#![no_std]
#![no_main]
#![feature(sync_unsafe_cell, naked_functions, asm_const, asm_sym)]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

const STACK_SIZE: usize = 0x4000;
#[repr(align(0x10))]
struct Stack([u8; STACK_SIZE]);
static STACK: core::cell::SyncUnsafeCell<Stack> = core::cell::SyncUnsafeCell::new(Stack([0u8; STACK_SIZE]));

#[naked]
#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    core::arch::asm!(
        "
        lea rsp, [{} + {}]
        call {}
        ",
        sym STACK,
        const STACK_SIZE,
        sym main,
        options(noreturn)
    )
}

extern "C" fn main() -> ! {
    loop {
        unsafe {
            core::arch::asm!(
                "
                push rdi

                mov rdi, 0x0
                int 0x30

                pop rdi
                ",
                options(nostack, nomem)
            );

            loop {}
        }
    }
}
