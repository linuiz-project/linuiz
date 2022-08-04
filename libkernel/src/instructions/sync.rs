#[cfg(target_arch = "x86_64")]
#[inline]
pub fn mfence() {
    unsafe { core::arch::asm!("mfence", options(nostack, nomem, preserves_flags)) };
}
