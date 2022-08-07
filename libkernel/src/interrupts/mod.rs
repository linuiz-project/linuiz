mod x64;

#[cfg(target_arch = "x86_64")]
pub use x64::*;

use core::cell::SyncUnsafeCell;
use x86_64::structures::idt::InterruptStackFrame;

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
