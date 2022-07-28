#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ID {
    Test,
}

#[repr(C)]
#[derive(Debug)]
pub struct Control {
    pub id: ID,
    pub blah: u64,
}

#[repr(u64)]
pub enum Error {
    ControlNotMapped,
}

pub fn syscall_interrupt_handler(
    isf: &mut x86_64::structures::idt::InterruptStackFrame,
    gprs: &mut crate::ThreadRegisters,
) {
    let control_ptr = gprs.rdi as *mut Control;

    if !crate::memory::global_pmgr()
        .is_mapped(crate::Address::<crate::Virtual>::from_ptr(control_ptr))
    {
        gprs.rsi = Error::ControlNotMapped as u64;
        return;
    }

    gprs.rsi = 0xD3ADC0DA;
}
