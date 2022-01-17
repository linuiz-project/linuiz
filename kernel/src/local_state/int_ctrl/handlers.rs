use libstd::structures::idt::InterruptStackFrame;

pub extern "x86-interrupt" fn apit_handler(_: InterruptStackFrame) {
    crate::local_state::clock().unwrap().tick();
    crate::local_state::int_ctrl().unwrap().end_of_interrupt();
}

pub extern "x86-interrupt" fn storage_handler(_: InterruptStackFrame) {
    // TODO somehow notify storage driver of interrupt
    crate::local_state::int_ctrl().unwrap().end_of_interrupt();
}

pub extern "x86-interrupt" fn error_handler(_: InterruptStackFrame) {
    let apic = &crate::local_state::int_ctrl().unwrap().apic;

    error!("APIC ERROR INTERRUPT");
    error!("--------------------");
    error!("DUMPING APIC ERROR REGISTER:");
    error!("  {:?}", apic.error_status());

    apic.end_of_interrupt();
}

pub extern "x86-interrupt" fn spurious_handler(_: InterruptStackFrame) {
    // Perhaps don't need to EOI spurious interrupts?
    // crate::lpu::try_get().unwrap().int_ctrl().end_of_interrupt();
}
