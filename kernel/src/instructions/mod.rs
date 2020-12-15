pub mod interrupts;

pub fn hlt() {
    unsafe {
        asm!("hlt", options(nomem, nostack));
    }
}
