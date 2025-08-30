use num_enum::{IntoPrimitive, TryFromPrimitive};

pub mod exceptions;
pub mod syscall;

#[repr(u8)]
#[derive(Debug, TryFromPrimitive, IntoPrimitive, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
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
}

/// Enables interrupts for the current processor.
pub fn enable() {
    #[cfg(target_arch = "x86_64")]
    crate::arch::x86_64::instructions::__sti();

    #[cfg(not(any(target_arch = "x86_64")))]
    unimplemented!();
}

/// Disables interrupts for the current processor.
pub fn disable() {
    #[cfg(target_arch = "x86_64")]
    crate::arch::x86_64::instructions::__cli();

    #[cfg(not(any(target_arch = "x86_64")))]
    unimplemented!();
}

/// Whether or not interrupts are enabled for the current processor.
pub fn is_enabled() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::registers::ProcessorFlags::read()
            .contains(crate::arch::x86_64::registers::ProcessorFlags::INTERRUPT_FLAG)
    }

    #[cfg(not(any(target_arch = "x86_64")))]
    {
        unimplemented!()
    }
}

/// Waits for the next interrupt on the current processor.
pub fn wait_next() {
    #[cfg(target_arch = "x86_64")]
    crate::arch::x86_64::instructions::__hlt();

    #[cfg(not(any(target_arch = "x86_64")))]
    unimplemented!();
}

/// Disables interrupts if they were enabled, executes `func`, then re-enables
/// interrupts if they were disabled.
#[inline]
pub fn uninterruptable<T>(func: impl FnOnce() -> T) -> T {
    cfg_select! {
        test => { func() }

        not(test) => {
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
    }
}

/// Indefinitely waits for the next interrupt on the current processor.
pub fn wait_indefinite() -> ! {
    loop {
        wait_next();
    }
}
