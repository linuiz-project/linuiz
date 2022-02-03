use core::arch::asm;

/// Initiates a breakpoint exception.
#[inline(always)]
pub fn breakpoint() {
    unsafe {
        asm!("int3");
    }
}

/// Enables interrupts via `sti`.
#[inline(always)]
pub fn enable() {
    unsafe {
        asm!("sti", options(nostack, nomem));
    }
}

/// Disables interrupts via `cli`.
#[inline(always)]
pub fn disable() {
    unsafe {
        asm!("cli", options(nostack, nomem));
    }
}

pub fn are_enabled() -> bool {
    use crate::registers::RFlags;

    RFlags::read().contains(RFlags::INTERRUPT_FLAG)
}

/// Executes given function with interrupts disabled, then
/// re-enables interrupts if they were previously enabled.
pub fn without_interrupts<R>(function: impl FnOnce() -> R) -> R {
    let interrupts_enabled = are_enabled();

    if interrupts_enabled {
        disable();
    }

    let return_value = function();

    if interrupts_enabled {
        enable();
    }

    return_value
}
