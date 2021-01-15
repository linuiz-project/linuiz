use x86_64::VirtAddr;

pub fn invalidate(addr: VirtAddr) {
    unsafe {
        asm!("invlpg [{}]", in(reg) addr.as_u64(), options(nostack));
    }
}

pub fn invalidate_all() {
    let (frame, flags) = crate::registers::CR3::read();
    unsafe { crate::registers::CR3::write(&frame, flags) };
}
