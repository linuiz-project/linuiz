pub fn handler(_: &mut x86_64::structures::idt::InterruptStackFrame, gprs: &mut crate::scheduling::ThreadRegisters) {
    let control_ptr = gprs.rdi as *mut libkernel::syscall::Control;

    if !crate::memory::get_kernel_page_manager()
        .unwrap()
        .is_mapped(libarch::Address::<libarch::Virtual>::from_ptr(control_ptr))
    {
        gprs.rsi = libkernel::syscall::Error::ControlNotMapped as u64;
        return;
    }

    gprs.rsi = 0xDEADC0DE;
}
