pub mod sync;
pub mod tlb;

/// Calls a breakpoint exception.
#[inline]
#[cfg(target_arch = "x86_64")]
pub fn breakpoint() {
    // Safety: Control flow expects to enter the breakpoint exception handler.
    unsafe {
        core::arch::asm!("int3", options(nostack, nomem));
    }
}
