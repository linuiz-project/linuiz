mod instructions;
pub use instructions::*;

use crate::cpu::{ArchContext, ControlContext};
use libsys::{Address, Virtual};
use num_enum::TryFromPrimitive;

/// Delivery mode for IPIs.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptDeliveryMode {
    Fixed = 0b000,
    LowPriority = 0b001,
    SMI = 0b010,
    NMI = 0b100,
    INIT = 0b101,
    StartUp = 0b110,
    ExtINT = 0b111,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum DeliveryMode {
    Fixed = 0b000,
    LowPriority = 0b001,
    SMI = 0b010,
    NMI = 0b100,
    INIT = 0b101,
    StartUp = 0b110,
    ExtINT = 0b111,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationMode {
    Physical = 0,
    Logical = 1,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[allow(non_camel_case_types)]
pub enum Vector {
    Clock = 0x20,
    /* 0x21..=0x2F reserved for PIC */
    Timer = 0x30,
    Thermal = 0x32,
    Performance = 0x33,
    /* 0x34..=0x3B free for use */
    Error = 0x3C,
    LINT0 = 0x3D,
    LINT1 = 0x3E,
    SPURIOUS = 0x3F,
}

/// Indicates what type of error the common page fault handler encountered.
#[derive(Debug, Clone, Copy)]
pub struct PageFaultHandlerError;

/// ### Safety
///
/// Do not call this function.
#[no_mangle]
#[repr(align(0x10))]
pub unsafe fn pf_handler(address: Address<Virtual>) -> Result<(), PageFaultHandlerError> {
    crate::local_state::with_address_space(|addr_space| {
        addr_space.demand_map(address).map_err(|_| PageFaultHandlerError)
    })
    .ok_or(PageFaultHandlerError)
    .flatten()
}

/// ### Safety
///
/// Do not call this function.
#[no_mangle]
#[repr(align(0x10))]
pub unsafe fn irq_handler(irq_vector: u64, ctrl_flow_context: &mut ControlContext, arch_context: &mut ArchContext) {
    match Vector::try_from(irq_vector) {
        Ok(Vector::Timer) => crate::local_state::next_task(ctrl_flow_context, arch_context),

        _vector_result => {}
    }

    #[cfg(target_arch = "x86_64")]
    crate::local_state::end_of_interrupt();
}

pub struct InterruptCell<T>(T);

impl<T> InterruptCell<T> {
    #[inline]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    #[inline]
    pub fn set(&mut self, value: T) {
        self.0 = value;
    }

    #[inline]
    pub fn with<U>(&self, func: impl FnOnce(&T) -> U) -> U {
        let value_ref = &self.0;
        without(|| func(value_ref))
    }

    #[inline]
    pub fn with_mut<U>(&mut self, func: impl FnOnce(&mut T) -> U) -> U {
        let value_mut = &mut self.0;
        without(|| func(value_mut))
    }
}
