pub mod interrupt_vector;

pub fn halt_until_interrupt_indefinite() -> ! {
    loop {
        crate::instructions::hlt();
    }
}