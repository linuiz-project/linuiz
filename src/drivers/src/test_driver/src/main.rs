#![no_std]
#![no_main]
#![feature(raw_ref_op, sync_unsafe_cell, asm_const, asm_sym, naked_functions)]

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
unsafe extern "C" fn _entry() -> ! {
    core::arch::asm!(
        "
        lea rsp, [{} + {}]
        call {}
        ",
        const STACK_SIZE,
        sym STACK,
        sym main,
        options(noreturn)
    );
}

extern "C" fn main() -> ! {
    let control = (0_u64, 0xD3ADC0D3_u64);

    loop {
        unsafe {
            let _result: u64;

            core::arch::asm!(
                "syscall",
                in("rdi") &raw const control,
                out("rsi") _result
            );

            loop {}
        }
    }
}
