pub fn htl_indefinite() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
