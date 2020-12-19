pub fn hlt() {
    unsafe {
        asm!("hlt", options(nomem, nostack));
    }
}

pub fn htl_indefinite() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
