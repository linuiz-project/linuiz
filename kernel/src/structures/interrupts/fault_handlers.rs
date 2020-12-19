use crate::{instructions::htl_indefinite, writeln};
use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};

pub(super) extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame) {
    writeln!("CPU EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

pub(super) extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("CPU EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

pub(super) extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    writeln!("CPU EXCEPTION: PAGE FAULT");
    writeln!("Accessed Address: {:?}", Cr2::read());
    writeln!("Error Code: {:?}", error_code);
    writeln!("{:#?}", stack_frame);

    htl_indefinite();
}
