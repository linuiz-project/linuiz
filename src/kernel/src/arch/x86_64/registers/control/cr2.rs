use libsys::{Address, Virtual};

pub struct CR2;

impl CR2 {
    /// Read the current page fault linear address from the CR2 register.
    pub fn read() -> Address<Virtual> {
        let value: u64;

        // Safety: Reading CR2 has no side effects.
        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags));
        }

        Address::new(usize::try_from(value).unwrap()).unwrap()
    }
}
