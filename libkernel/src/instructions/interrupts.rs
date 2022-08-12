/// Enables interrupts for the current core.
///
/// SAFETY: Enabling interrupts early can result in unexpected behaviour.
#[inline(always)]
pub unsafe fn enable() {
    core::arch::asm!("sti", options(nostack, nomem));
}

/// Disables interrupts for the current core.
///
/// SAFETY: Disabling interrupts can cause the system to become unresponsive if they are not re-enabled.
#[inline(always)]
pub unsafe fn disable() {
    core::arch::asm!("cli", options(nostack, nomem));
}

/// Returns whether or not interrupts are enabled for the current core.
#[inline(always)]
pub fn are_enabled() -> bool {
    use crate::registers::RFlags;

    RFlags::read().contains(RFlags::INTERRUPT_FLAG)
}

/// Disables interrupts, executes the given [`FnOnce`], and re-enables interrupts if they were prior.
pub fn without_interrupts<R>(func: impl FnOnce() -> R) -> R {
    let interrupts_enabled = are_enabled();

    if interrupts_enabled {
        unsafe { disable() };
    }

    let return_value = func();

    if interrupts_enabled {
        unsafe { enable() };
    }

    return_value
}

/// Waits for the next interrupt on the current core.
#[inline(always)]
pub fn wait() {
    unsafe {
        core::arch::asm!("hlt", options(nostack, nomem, preserves_flags));
    }
}

/// Indefinitely waits for the next interrupt on the current core.
#[inline(always)]
pub fn wait_indefinite() -> ! {
    loop {
        wait()
    }
}
