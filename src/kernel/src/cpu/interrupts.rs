/// Enables interrupts for the current hardware thread.
pub fn enable() {
    #[cfg(any(target_arch = "x86_64"))]
    crate::arch::x86_64::instructions::__sti();

    #[cfg(not(any(target_arch = "x86_64")))]
    unimplemented!();
}

/// Disables interrupts for the current hardware thread.
pub fn disable() {
    #[cfg(any(target_arch = "x86_64"))]
    crate::arch::x86_64::instructions::__cli();

    #[cfg(not(any(target_arch = "x86_64")))]
    unimplemented!();
}

/// Whether or not interrupts are enabled for the current hardware thread.
pub fn is_enabled() -> bool {
    #[cfg(any(target_arch = "x86_64"))]
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
    #[cfg(any(target_arch = "x86_64"))]
    crate::arch::x86_64::instructions::__hlt();

    #[cfg(not(any(target_arch = "x86_64")))]
    unimplemented!();
}
