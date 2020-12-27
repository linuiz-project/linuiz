use crate::{serialln, structures::pic::PICInterrupt};
use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

/* FAULT INTERRUPT HANDLERS */

extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame) {
    serialln!("CPU EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("CPU EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    serialln!("CPU EXCEPTION: PAGE FAULT");
    serialln!(
        "Accessed Address: {:?}",
        x86_64::registers::control::Cr2::read()
    );
    serialln!("Error Code: {:?}", error_code);
    serialln!("{:#?}", stack_frame);

    crate::instructions::htl_indefinite();
}

/* REGULAR INTERRUPT HANDLERS */

extern "x86-interrupt" fn timer_interrupt_handler(_: &mut InterruptStackFrame) {
    crate::serial!(".");
    crate::structures::pic::end_of_interrupt(PICInterrupt::Timer);
}

/* IDT */

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
        //idt[PICInterrupt::Timer.into()].set_handler_fn(timer_interrupt_handler);

        idt
    };
}

pub fn init() {
    IDT.load();
}
