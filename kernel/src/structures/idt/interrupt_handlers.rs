use crate::structures::{
    idt::InterruptStackFrame,
    pic::{end_of_interrupt, PICInterrupt},
};

pub(super) extern "x86-interrupt" fn timer_interrupt_handler(_: &mut InterruptStackFrame) {
    crate::serial!(".");
    end_of_interrupt(PICInterrupt::Timer);
}
