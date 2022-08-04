mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

use crate::{Address, Virtual};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Vector {
    GlobalTimer = 0x20,
    Error = 0x40,
    LocalTimer = 0x41,
    Performance = 0x46,
    ThermalSensor = 0x47,
    Storage0 = 0x50,
    Storage1 = 0x51,
    Storage2 = 0x52,

    Syscall = 0x80,

    /* CANNOT BE CHANGED — DEFAULT FROM APIC */
    LINT0_VECTOR = 253,
    LINT1_VECTOR = 254,
    SPURIOUS_VECTOR = 255,
}

#[cfg(target_arch = "x86_64")]
pub type HandlerFunc = fn(&mut InterruptStackFrame, &mut crate::cpu::ThreadRegisters);

static INTERRUPT_HANDLERS: spin::RwLock<[Option<HandlerFunc>; 256]> = spin::RwLock::new([None; 256]);


/// Sets the interrupt handler function for the given vector.
///
/// SAFETY: This function is unsafe because any (including a malformed or buggy) handler can be
///         specified. The caller of this function must ensure the handler is correctly formed,
///         and properly handles the interrupt it is being assigned to.  
pub unsafe fn set_handler_fn(vector: Vector, handler: HandlerFunc) {
    crate::instructions::interrupts::without_interrupts(|| {
        INTERRUPT_HANDLERS.write()[vector as usize] = Some(handler);
    });
}

pub struct InterruptRequest {
    fn_addr: Address<Virtual>,
}

pub struct InterruptResponse {}

pub trait InterruptController {
    fn new() -> Self;

    fn init();

    fn submit_interrupt_request(fn_addr: Address<Virtual>) -> usize;
}
