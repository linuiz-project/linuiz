use lzstd::Ptr;

pub struct CR2;

impl CR2 {
    /// Read the current page fault linear address from the CR2 register.
    pub fn read() -> Ptr<u8> {
        let value: *mut u8;

        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags));
        }

        Ptr::try_from(value)
    }
}
