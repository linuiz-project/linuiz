pub mod exceptions;
pub mod syscall;

#[repr(u8)]
#[derive(Debug, FromPrimitive, IntoPrimitive, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Vector {
    Watchdog = 0x20,
    Timer = 0x21,
    Error = 0x22,
    PerformanceCounter = 0x23,
    ThermalSensor = 0x24,
    CMCI = 0x25,
    External = 0x26,

    Syscall = 0x80,

    Spurious = 0xFF,

    #[default]
    Unknown = 0,
}

/// Enables interrupts for the current hardware thread.
pub fn enable() {
    #[cfg(target_arch = "x86_64")]
    crate::arch::x86_64::instructions::__sti();

    #[cfg(not(any(target_arch = "x86_64")))]
    unimplemented!();
}

/// Disables interrupts for the current hardware thread.
pub fn disable() {
    #[cfg(target_arch = "x86_64")]
    crate::arch::x86_64::instructions::__cli();

    #[cfg(not(any(target_arch = "x86_64")))]
    unimplemented!();
}

/// Whether or not interrupts are enabled for the current hardware thread.
pub fn is_enabled() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::registers::RFlags::read()
            .contains(crate::arch::x86_64::registers::RFlags::INTERRUPT_FLAG)
    }

    #[cfg(not(any(target_arch = "x86_64")))]
    {
        unimplemented!()
    }
}

/// Waits for the next interrupt on the current hardware thread.
pub fn wait_next() {
    #[cfg(target_arch = "x86_64")]
    crate::arch::x86_64::instructions::__hlt();

    #[cfg(not(any(target_arch = "x86_64")))]
    unimplemented!();
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
        uninterruptable(|| func(&self.0))
    }

    #[inline]
    pub fn with_mut<U>(&mut self, func: impl FnOnce(&mut T) -> U) -> U {
        uninterruptable(|| func(&mut self.0))
    }
}

/// Disables interrupts if they were enabled, executes `func`, then re-enables interrupts if they were disabled.
#[inline]
pub fn uninterruptable<T>(func: impl FnOnce() -> T) -> T {
    let interrupts_enabled = is_enabled();

    if interrupts_enabled {
        disable();
    }

    let return_value = func();

    if interrupts_enabled {
        enable();
    }

    return_value
}

/// Indefinitely waits for the next interrupt on the current hardware thread.
pub fn wait_indefinite() -> ! {
    loop {
        wait_next();
    }
}
