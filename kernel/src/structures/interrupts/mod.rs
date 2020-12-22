mod fault_handlers;
mod interrupt_handlers;
pub mod interrupt_vector;

use bitflags::bitflags;
use fault_handlers::*;
use interrupt_handlers::*;
use lazy_static::lazy_static;
use crate::structures::pic::InterruptOffset;

bitflags! {
    #[repr(transparent)]
    pub struct PageFaultError: u64 {
        const PROTECTION_VIOLATION = 1;
        const CAUSED_BY_WRITE= 1 << 1;
        const USER_MODE = 1 << 2;
        const MALFORMED_TABLE = 1 << 3;
        const INSTRUCTION_FETCH = 1 << 4;
    }
}


#[repr(C)]
#[repr(align(16))]
pub struct InterruptDescriptorTable {
    pub divide_error: 
}





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
