/// Calls a breakpoint exception.
#[inline]
#[cfg(target_arch = "x86_64")]
pub fn breakpoint() {
    // Safety: Control flow expects to enter the breakpoint exception handler.
    unsafe {
        core::arch::asm!("int3", options(nostack, nomem));
    }
}

pub mod sync {
    #[inline]
    pub fn mfence() {
        // Safety: `mfence` does not have instruction side effects.
        unsafe { core::arch::asm!("mfence", options(nostack, nomem, preserves_flags)) };
    }
}

pub mod tlb {
    use libsys::{Address, Page};

    /// Invalidates a single page from the TLB.
    #[inline]
    pub fn invlpg(page: Address<Page>) {
        // Safety: Invalidating a page from the cache has no program side effects.
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) page.get().get(), options(nostack, preserves_flags));
        }
    }
}
