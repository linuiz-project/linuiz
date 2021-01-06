use crate::{serialln, structures::pic::InterruptOffset};
use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

/* FAULT INTERRUPT HANDLERS */
extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame) {
    serialln!("CPU EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: INVALID OPCODE\n{:#?}", stack_frame);
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

    crate::instructions::hlt_indefinite();
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    #[repr(u8)]
    #[derive(Debug)]
    enum IndexType {
        GDT = 0b00,
        IDT0 = 0b01,
        LDT = 0b10,
        IDT1 = 0b11,
    }

    impl IndexType {
        fn from_u8(value: u8) -> Self {
            match value {
                x if x == (IndexType::GDT as u8) => IndexType::GDT,
                x if x == (IndexType::IDT0 as u8) => IndexType::IDT0,
                x if x == (IndexType::LDT as u8) => IndexType::LDT,
                x if x == (IndexType::IDT1 as u8) => IndexType::IDT1,
                _ => panic!("invalid selector index type!"),
            }
        }
    }

    let external = (error_code & 0x1) != 0x0;
    let selector_index_type = IndexType::from_u8(((error_code >> 1) & 0x3) as u8);
    let selector_index = (error_code >> 3) & 0x1FFF;

    panic!(
        "CPU EXCEPTION: GENERAL PROTECTION FAULT:\n External: {}\n IndexType: {:?}\n Index: {}\n {:#?}",
        external, selector_index_type, selector_index, stack_frame
    );
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("CPU EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

/* REGULAR INTERRUPT HANDLERS */
extern "x86-interrupt" fn timer_interrupt_handler(_: &mut InterruptStackFrame) {
    crate::structures::pic::end_of_interrupt(InterruptOffset::Timer);
}

/* IDT */
lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // fault interrupts
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);

        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler).set_stack_index(crate::structures::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        // regular interrupts
        idt[InterruptOffset::Timer as usize].set_handler_fn(timer_interrupt_handler);

        idt
    };
}

pub fn init() {
    IDT.load();
}
