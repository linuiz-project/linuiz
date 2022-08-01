pub mod interrupts;
pub mod pwm;
pub mod sync;

/// Simple wait-one instruction.
#[inline(always)]
pub fn pause() {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            core::arch::asm!("pause", options(nostack, nomem, preserves_flags));
        }
    }
}
