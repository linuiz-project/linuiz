use crate::memory::Page;

pub fn invalidate(page: &Page) {
    unsafe {
        asm!("invlpg [{}]", in(reg) page.base_addr().as_usize(), options(nostack));
    }
}

pub fn invalidate_all() {
    crate::registers::CR3::refresh();
}
