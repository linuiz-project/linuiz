use crate::{
    interrupts::Vector,
    proc::{Registers, State},
};
use libsys::{Address, Virtual};

/// Indicates what type of error the common page fault handler encountered.
#[derive(Debug, Clone, Copy)]
pub struct PageFaultHandlerError;

/// ### Safety
///
/// This function should only be called in the case of passing context to handle a page fault.
/// Calling this function more than once and/or outside the context of a page fault is undefined behaviour.
#[doc(hidden)]
#[repr(align(0x10))]
pub unsafe fn pf_handler(address: Address<Virtual>) -> Result<(), PageFaultHandlerError> {
    crate::local::with_current_address_space(|addr_space| {
        trace!("Processing page fault for address: {:X?}", address);
        addr_space.try_demand(Address::new_truncate(address.get())).ok()
    })
    .flatten()
    .ok_or(PageFaultHandlerError)
}

/// ### Safety
///
/// This function should only be called in the case of passing context to handle an interrupt.
/// Calling this function more than once and/or outside the context of an interrupt is undefined behaviour.
#[doc(hidden)]
#[repr(align(0x10))]
pub unsafe fn irq_handler(irq_vector: u64, state: &mut State, regs: &mut Registers) {
    match Vector::try_from(irq_vector) {
        Ok(Vector::Timer) => crate::local::next_task(state, regs),

        Err(err) => panic!("Invalid interrupt vector: {:X?}", err),
        vector_result => unimplemented!("Unhandled interrupt: {:?}", vector_result),
    }

    #[cfg(target_arch = "x86_64")]
    crate::local::end_of_interrupt();
}
