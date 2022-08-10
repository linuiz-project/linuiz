mod x64;

#[cfg(target_arch = "x86_64")]
pub use x64::*;

use core::cell::SyncUnsafeCell;
use libkernel::cpu::GeneralRegisters;
use num_enum::TryFromPrimitive;
use x86_64::structures::idt::InterruptStackFrame;

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[allow(non_camel_case_types)]
pub enum Vector {
    Syscall = 0x80,
    Timer = 0xA0,

    /* 224-256 ARCH SPECIFIC */
}

pub fn common_interrupt_handler(
    irq_vector: u64,
    stack_frame: &mut x86_64::structures::idt::InterruptStackFrame,
    context: &mut GeneralRegisters,
) {
    match Vector::try_from(irq_vector) {
        Ok(vector) => match vector {
            Vector::Syscall => todo!(),
            Vector::Timer => todo!(),
            Vector::Performance => todo!(),
            Vector::ThermalSensor => todo!(),
            Vector::Error => todo!(),
            Vector::LINT0_VECTOR | Vector::LINT1_VECTOR | Vector::SPURIOUS_VECTOR => {}
        },
        Err(vector_raw) => warn!("Unhandled IRQ vector: {:?}", vector_raw),
    }

    // TODO abstract this
    libkernel::structures::apic::end_of_interrupt();
}

/* EXCEPTION HANDLING */
type ExceptionHandler = fn(Exception);
static EXCEPTION_HANDLER: SyncUnsafeCell<ExceptionHandler> =
    SyncUnsafeCell::new(|exception| panic!("\n{:#?}", exception));

/// Sets the common exception handler.
///
/// SAFETY: The caller must ensure the provided function handles exceptions in a valid way.
pub unsafe fn set_common_exception_handler(handler: ExceptionHandler) {
    *EXCEPTION_HANDLER.get() = handler;
}

/// Gets the current common exception handler.
#[inline]
pub(self) fn get_common_exception_handler() -> &'static ExceptionHandler {
    unsafe { &*EXCEPTION_HANDLER.get() }
}

/* NON-EXCEPTION IRQ HANDLING */
type InterruptHandler = fn(u64, &mut InterruptStackFrame, &mut GeneralRegisters);
static INTERRUPT_HANDLER: SyncUnsafeCell<InterruptHandler> =
    SyncUnsafeCell::new(|irq_num, _, _| panic!("IRQ{}: no common handler", irq_num));

/// Sets the common interrupt handler.
///
/// SAFETY: The caller must ensure the provided function handles interrupts in a valid way.
pub unsafe fn set_common_interrupt_handler(handler: InterruptHandler) {
    *INTERRUPT_HANDLER.get() = handler;
}

/// Gets the current common interrupt handler.
#[inline]
pub(self) fn get_common_interrupt_handler() -> &'static InterruptHandler {
    unsafe { &*INTERRUPT_HANDLER.get() }
}
