use crate::memory::Page;

pub fn invalidate(page: &Page) {
    unsafe {
        asm!("invlpg [{}]", in(reg) page.addr().as_usize(), options(nostack));
    }
}

pub fn invalidate_all() {
    crate::registers::CR3::refresh();
}
