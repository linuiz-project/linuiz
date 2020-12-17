use crate::boot::pic::{end_of_interrupt, InterruptOffset};
use x86_64::structures::idt::InterruptStackFrame;

pub extern "x86-interrupt" fn timer_interrupt_handler(_: &mut InterruptStackFrame) {
    end_of_interrupt(InterruptOffset::Timer);
}
