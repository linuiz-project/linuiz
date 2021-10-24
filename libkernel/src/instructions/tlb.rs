use crate::memory::Page;

#[inline]
pub fn invalidate(page: &Page) {
    unsafe {
        asm!("invlpg [{}]", in(reg) page.base_addr().as_usize(), options(nostack));
    }
}

#[inline]
pub fn invalidate_all() {
    crate::registers::CR3::refresh();
}
