pub fn mfence() {
    // Safety: `mfence` does not have instruction side effects.
    unsafe { core::arch::asm!("mfence", options(nostack, nomem, preserves_flags)) };
}
