use crate::pic::{end_of_interrupt, InterruptOffset};
use lazy_static::lazy_static;

/* FAULT HANDLERS */

/* INTERRUPT HANDLERS */

/* IDT */

pub fn halt_until_interrupt_indefinite() -> ! {
    loop {
        crate::instructions::hlt();
    }
}
