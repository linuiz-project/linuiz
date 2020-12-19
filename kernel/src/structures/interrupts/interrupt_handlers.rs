use crate::{
    structures::pic::{end_of_interrupt, InterruptOffset},
    write,
};
use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptStackFrame;

pub(super) extern "x86-interrupt" fn timer_interrupt_handler(_: &mut InterruptStackFrame) {
    end_of_interrupt(InterruptOffset::Timer);
}