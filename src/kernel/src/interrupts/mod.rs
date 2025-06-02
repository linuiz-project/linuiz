use num_enum::TryFromPrimitive;

pub mod exceptions;
pub mod syscall;

#[cfg(target_arch = "x86_64")]
pub use crate::arch::x86_64::instructions::interrupts::{disable, enable, is_enabled, wait_next};

/// Delivery mode for IPIs.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[allow(clippy::upper_case_acronyms)]
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
#[allow(clippy::upper_case_acronyms)]
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
    Spurious = 0x3F,

    Syscall = 0x80,
}

/// Provides access to the contained instance of `T`, ensuring interrupts are disabled for the duration of the borrow.
pub struct InterruptCell<T>(T);

impl<T> InterruptCell<T> {
    #[inline]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    #[inline]
    pub fn with<U>(&self, func: impl FnOnce(&T) -> U) -> U {
        without(|| func(&self.0))
    }

    #[inline]
    pub fn with_mut<U>(&mut self, func: impl FnOnce(&mut T) -> U) -> U {
        without(|| func(&mut self.0))
    }
}

/// Disables interrupts, executes the given [`FnOnce`], and re-enables interrupts if they were prior.
#[inline]
pub fn without<R>(func: impl FnOnce() -> R) -> R {
    let interrupts_enabled = is_enabled();

    if interrupts_enabled {
        // Safety: Interrupts are expected to be disabled, and are later re-enabled.
        unsafe {
            disable();
        }
    }

    let return_value = func();

    if interrupts_enabled {
        // Safety: Interrupts were previously enabled.
        unsafe {
            enable();
        }
    }

    return_value
}

/// Indefinitely waits for the next interrupt on the current hardware thread.
#[inline(always)]
pub fn wait_indefinite() -> ! {
    loop {
        // Safety: We never recover from this, so it doesn't matter if a deadlock occurs.
        unsafe {
            wait_next();
        }
    }
}

/// Murder, in cold electrons, the current hardware thread.
///
/// ## Safety
///
/// Murdering a hardware thread is undefined behaviour.
#[inline(always)]
pub unsafe fn halt_and_catch_fire() -> ! {
    // Safety: We never recover from this, so it doesn't matter.
    unsafe {
        disable();
    }

    wait_indefinite()
}
