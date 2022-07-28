use crate::memory::Page;

#[inline]
pub fn invalidate(page: &Page) {
    unsafe {
        core::arch::asm!("invlpg [{}]", in(reg) page.base_addr().as_usize(), options(nostack));
    }
}

#[inline]
pub fn invalidate_all() {
    crate::registers::control::CR3::refresh();
}
