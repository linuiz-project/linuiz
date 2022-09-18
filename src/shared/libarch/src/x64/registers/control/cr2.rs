use libcommon::{Address, Virtual};

pub struct CR2;

impl CR2 {
    /// Read the current page fault linear address from the CR2 register.
    pub fn read() -> Address<Virtual> {
        let value: u64;

        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags));
        }

        Address::<Virtual>::new_truncate(value)
    }
}
