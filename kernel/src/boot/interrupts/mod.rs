mod fault_handlers;
mod interrupt_handlers;

use interrupt_handlers::*;
use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;

use super::pic::InterruptOffset;

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
