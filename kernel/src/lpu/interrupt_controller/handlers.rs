use super::InterruptVector;
use core::sync::atomic::Ordering;
use libstd::structures::idt::InterruptStackFrame;

static INT_NO_LPU: &str = "Interrupts enabled without system clock (no LPU structure)";

pub extern "x86-interrupt" fn apit_handler(_: InterruptStackFrame) {
    crate::println!("s");
    if let Some(lpu) = crate::lpu::try_get() {
        lpu.clock.tick();
        lpu.int_ctrl.counters[InterruptVector::Timer as usize].fetch_add(1, Ordering::Relaxed);
        lpu.int_ctrl.end_of_interrupt();
    }
    crate::println!("e");
}

pub extern "x86-interrupt" fn storage_handler(_: InterruptStackFrame) {
    crate::lpu::try_get().expect(INT_NO_LPU).int_ctrl.counters[InterruptVector::Storage as usize]
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
