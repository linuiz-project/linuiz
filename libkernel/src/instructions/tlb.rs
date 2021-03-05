use crate::memory::Page;

pub fn invalidate(page: &Page) {
    unsafe {
        asm!("invlpg [{}]", in(reg) page.addr_u64(), options(nostack));
    }
}

pub fn invalidate_all() {
    crate::registers::CR3::refresh();
}
