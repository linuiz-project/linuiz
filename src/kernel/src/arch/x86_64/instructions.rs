use core::arch::asm;
use libsys::{Address, Page};

#[derive(Debug, Error)]
pub enum Error {
    #[error("instruction is not supported on the current platform")]
    InstructionSupport,
}

pub fn __rdrand() -> Result<u64, Error> {
    todo!()
}

pub fn __rdseed() -> Result<u64, Error> {
    todo!()
}

/// Enables interrupts for the current hardware thread.
#[inline(always)]
pub fn __sti() {
    // Safety: Caller is required to ensure enabling interrupts will not cause undefined behaviour.
    unsafe {
        asm!("sti", options(nostack, nomem));
    }
}

/// Disables interrupts for the current hardware thread.
#[inline(always)]
pub fn __cli() {
    // Safety: Caller is required to ensure disabling interrupts will not cause undefined behaviour.
    unsafe {
        asm!("cli", options(nostack, nomem));
    }
}

/// Waits for the next interrupt on the current hardware thread.
pub fn __hlt() {
    // Safety: Caller must guarantee this does not cause a deadlock.
    unsafe {
        asm!("hlt", options(nostack, nomem, preserves_flags));
    }
}

/// Invalidates a single page from the TLB (translation look-aside buffer).
#[inline(always)]
pub fn __invlpg(page: Address<Page>) {
    // Safety: Invalidating a page from the cache has no program side effects.
    unsafe {
        core::arch::asm!("invlpg [{}]", in(reg) page.get().get(), options(nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn __mfence() {
    // Safety: `mfence` does not have instruction side effects.
    unsafe {
        core::arch::asm!("mfence", options(nostack, nomem, preserves_flags));
    }
}
