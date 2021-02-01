use x86_64::VirtAddr;

pub struct CR2;

impl CR2 {
    /// Read the current page fault linear address from the CR2 register.
    pub fn read() -> VirtAddr {
        let value: u64;

        unsafe {
            asm!("mov {}, cr2", out(reg) value, options(nomem, nostack));
        }

        VirtAddr::new(value)
    }
}
