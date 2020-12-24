use crate::{
    instructions::htl_indefinite,
    serialln,
    structures::idt::{InterruptStackFrame, PageFaultError},
};

pub(super) extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame) {
    serialln!("CPU EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

pub(super) extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("CPU EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

pub(super) extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: PageFaultError,
) {
    use crate::instructions::registers::CR2;

    serialln!("CPU EXCEPTION: PAGE FAULT");
    serialln!("Accessed Address: {:?}", CR2::read());
    serialln!("Error Code: {:?}", error_code);
    serialln!("{:#?}", stack_frame);

    htl_indefinite();
}
