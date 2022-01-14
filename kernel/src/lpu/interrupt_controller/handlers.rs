use super::InterruptVector;
use core::sync::atomic::Ordering;
use libstd::structures::idt::InterruptStackFrame;

pub extern "x86-interrupt" fn apit_handler(_: InterruptStackFrame) {
    let lpu = crate::lpu::try_get().expect("LPU not configured.");
    lpu.clock.tick();
    lpu.int_ctrl.counters[InterruptVector::LocalTimer as usize].fetch_add(1, Ordering::Relaxed);
    lpu.int_ctrl.end_of_interrupt();
}

pub extern "x86-interrupt" fn storage_handler(_: InterruptStackFrame) {
    crate::lpu::try_get().unwrap().int_ctrl.counters[InterruptVector::Storage as usize]
        .fetch_add(1, Ordering::Relaxed);
}

pub extern "x86-interrupt" fn error_handler(_: InterruptStackFrame) {
    let apic = &crate::lpu::try_get().unwrap().int_ctrl.apic;

    error!("APIC ERROR INTERRUPT");
    error!("--------------------");
    error!("DUMPING APIC ERROR REGISTER:");
    error!("  {:?}", apic.error_status());

    apic.end_of_interrupt();
}

pub extern "x86-interrupt" fn spurious_handler(_: InterruptStackFrame) {
    crate::lpu::try_get().unwrap().int_ctrl().end_of_interrupt();
}
