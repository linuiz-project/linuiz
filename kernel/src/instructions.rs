pub fn hlt_indefinite() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
