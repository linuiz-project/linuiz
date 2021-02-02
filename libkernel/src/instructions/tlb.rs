use crate::memory::Page;

pub fn invalidate(page: &Page) {
    unsafe {
        asm!("invlpg [{}]", in(reg) page.addr().as_u64(), options(nostack));
    }
}

pub fn invalidate_all() {
    let (frame, flags) = crate::registers::CR3::read();
    unsafe { crate::registers::CR3::write(&frame, flags) };
}