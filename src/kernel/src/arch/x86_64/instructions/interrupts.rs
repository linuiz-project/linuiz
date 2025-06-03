use core::arch::asm;

/// Enables interrupts for the current hardware thread.
///
/// ## Safety
///
/// Enabling interrupts early can result in unexpected behaviour.
#[inline]
pub unsafe fn enable() {
    // Safety: Caller is required to ensure enabling interrupts will not cause undefined behaviour.
    unsafe {
        asm!("sti", options(nostack, nomem));
    }
}

/// Disables interrupts for the current hardware thread.
///
/// ## Safety
///
/// Disabling interrupts can cause the system to become unresponsive if they are not re-enabled.
#[inline]
pub unsafe fn disable() {
    // Safety: Caller is required to ensure disabling interrupts will not cause undefined behaviour.
    unsafe {
        asm!("cli", options(nostack, nomem));
    }
}

/// Whether or not interrupts are enabled for the current hardware thread.
#[inline]
pub fn is_enabled() -> bool {
    crate::arch::x86_64::registers::RFlags::read()
        .contains(crate::arch::x86_64::registers::RFlags::INTERRUPT_FLAG)
}

/// Waits for the next interrupt on the current hardware thread.
///
/// ## Safety
///
/// If interrupts are not enabled, this function will cause a deadlock.
#[inline]
pub unsafe fn wait_next() {
    // Safety: Caller must guarantee this does not cause a deadlock.
    unsafe {
        asm!("hlt", options(nostack, nomem, preserves_flags));
    }
}
