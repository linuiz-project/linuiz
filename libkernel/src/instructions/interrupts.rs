/// Enables interrupts for the current core.
#[inline(always)]
pub fn enable() {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            core::arch::asm!("sti", options(nostack, nomem));
        }
    }
}

/// Disables interrupts for the current core.
#[inline(always)]
pub fn disable() {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            core::arch::asm!("cli", options(nostack, nomem));
        }
    }
}

/// Returns whether or not interrupts are enabled for the current core.
#[inline(always)]
pub fn are_enabled() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        use crate::registers::x86_64::RFlags;

        RFlags::read().contains(RFlags::INTERRUPT_FLAG)
    }
}

/// Disables interrupts, executes the given [`FnOnce`], and re-enables interrupts if they were prior.
pub fn without_interrupts<R>(func: impl FnOnce() -> R) -> R {
    let interrupts_enabled = are_enabled();

    if interrupts_enabled {
        disable();
    }

    let return_value = func();

    if interrupts_enabled {
        enable();
    }

    return_value
}

/// Waits for the next interrupt on the current core.
#[inline(always)]
pub fn wait() {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            core::arch::asm!("hlt", options(nostack, nomem, preserves_flags));
        }
    }
}

/// Indefinitely waits for the next interrupt on the current core.
#[inline(always)]
pub fn wait_indefinite() -> ! {
    loop {
        wait()
    }
}
