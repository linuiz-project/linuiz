use crate::pic::{end_of_interrupt, InterruptOffset};
use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

/* FAULT HANDLERS */

/* INTERRUPT HANDLERS */

extern "x86-interrupt" fn timer_interrupt_handler(_: &mut InterruptStackFrame) {
    end_of_interrupt(InterruptOffset::Timer);
}

/* IDT */

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt[InterruptOffset::Timer.into()].set_handler_fn(timer_interrupt_handler);

        idt
    };
}

pub fn load_idt() {
    IDT.load();
}

pub fn halt_until_interrupt_indefinite() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
