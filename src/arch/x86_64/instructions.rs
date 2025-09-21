use crate::mem::addr::virt::VirtualAddress;
use core::arch::asm;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("instruction is not supported on the current platform")]
    InstructionSupport,
}

/// Enables interrupts for the current processor.
#[inline(always)]
pub fn __sti() {
    // Safety: Caller is required to ensure enabling interrupts will not cause
    // undefined behaviour.
    unsafe {
        asm!("sti", options(nostack, nomem));
    }
}

/// Disables interrupts for the current processor.
#[inline(always)]
pub fn __cli() {
    // Safety: Caller is required to ensure disabling interrupts will not cause
    // undefined behaviour.
    unsafe {
        asm!("cli", options(nostack, nomem));
    }
}

/// Waits for the next interrupt on the current processor.
pub fn __hlt() {
    // Safety: Caller must guarantee this does not cause a deadlock.
    unsafe {
        asm!("hlt", options(nostack, nomem, preserves_flags));
    }
}

/// Invalidates a single page from the TLB (translation look-aside buffer).
#[inline(always)]
pub fn __invlpg(address: VirtualAddress) {
    // Safety: Invalidating a page from the cache has no program side effects.
    unsafe {
        core::arch::asm!(
            "invlpg [{}]",
            in(reg) usize::from(address),
            options(nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub fn __mfence() {
    // Safety: `mfence` does not have instruction side effects.
    unsafe {
        core::arch::asm!("mfence", options(nostack, nomem, preserves_flags));
    }
}
