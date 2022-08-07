use num_enum::TryFromPrimitive;

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[allow(non_camel_case_types)]
pub enum Vector {
    Syscall = 0x80,
    Timer = 0xA0,
    Performance = 0xA1,
    ThermalSensor = 0xA2,

    Error = 0xFC,

    /* CANNOT BE CHANGED — DEFAULT FROM APIC */
    LINT0_VECTOR = 0xFD,
    LINT1_VECTOR = 0xFE,
    SPURIOUS_VECTOR = 0xFF,
}

pub fn common_interrupt_handler(
    irq_vector: u64,
    stack_frame: &mut x86_64::structures::idt::InterruptStackFrame,
    context: &mut libkernel::interrupts::GeneralRegisters,
) {
    let vector = Vector::try_from(irq_vector).unwrap_or_else(|vector_raw| {
        warn!("Unhandled IRQ vector: {:?}", vector_raw);
        return;
    });

    match vector {
        Vector::Syscall => todo!(),
        Vector::Timer => todo!(),
        Vector::Performance => todo!(),
        Vector::ThermalSensor => todo!(),
        Vector::Error => todo!(),
        Vector::LINT0_VECTOR | Vector::LINT1_VECTOR | Vector::SPURIOUS_VECTOR => {}
    }

    // TODO abstract this
    libkernel::structures::apic::end_of_interrupt();
}
