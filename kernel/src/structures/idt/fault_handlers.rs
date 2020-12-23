use crate::structures::idt::{InterruptStackFrame, PageFaultError};
use crate::{instructions::htl_indefinite, writeln};

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
    error_code: PageFaultError,
) {
    use crate::instructions::registers::CR2;

    writeln!("CPU EXCEPTION: PAGE FAULT");
    writeln!("Accessed Address: {:?}", CR2::read());
    writeln!("Error Code: {:?}", error_code);
    writeln!("{:#?}", stack_frame);

    htl_indefinite();
}
