use crate::{
    interrupts::Vector,
    task::{Registers, State},
};

/// ### Safety
///
/// This function should only be called in the case of passing context to handle an interrupt.
/// Calling this function more than once and/or outside the context of an interrupt is undefined behaviour.
#[doc(hidden)]
#[inline(never)]
pub unsafe fn handle_trap(irq_vector: u64, state: &mut State, regs: &mut Registers) {
    match Vector::try_from(irq_vector) {
        Ok(Vector::Timer) => crate::local::with_scheduler(|scheduler| scheduler.interrupt_task(state, regs))
            .expect("local state is not initialized"),
        Err(err) => panic!("Invalid interrupt vector: {:X?}", err),
        vector_result => unimplemented!("Unhandled interrupt: {:?}", vector_result),
    }

    crate::local::end_of_interrupt().unwrap();
}
