#[cfg(target_arch = "x86_64")]
pub mod x86_64;

pub mod interrupts;
pub mod pwm;

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
