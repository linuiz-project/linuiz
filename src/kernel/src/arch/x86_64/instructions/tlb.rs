use libsys::{Address, Page};

/// Invalidates a single page from the TLB.
pub fn invlpg(page: Address<Page>) {
    // Safety: Invalidating a page from the cache has no program side effects.
    unsafe {
        core::arch::asm!("invlpg [{}]", in(reg) page.get().get(), options(nostack, preserves_flags));
    }
}
