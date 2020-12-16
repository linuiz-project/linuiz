pub mod interrupt_vector;

use crate::structures::pic::{end_of_interrupt, InterruptOffset};

pub fn halt_until_interrupt_indefinite() -> ! {
    loop {
        crate::instructions::hlt();
    }
}