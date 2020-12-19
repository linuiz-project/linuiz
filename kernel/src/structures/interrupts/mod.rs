mod fault_handlers;
mod interrupt_handlers;

use fault_handlers::*;
use interrupt_handlers::*;
use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;

use super::pic::InterruptOffset;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // fault interrupts
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);

        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler).set_stack_index(crate::structures::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        // regular interrupts
        idt[InterruptOffset::Timer.into()].set_handler_fn(timer_interrupt_handler);

        idt
    };
}

pub fn load_idt() {
    IDT.load();
}
