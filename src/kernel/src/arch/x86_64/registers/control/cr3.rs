use core::arch::asm;
use libsys::{Address, Frame};

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CR3Flags: u64 {
        const PAGE_LEVEL_WRITE_THROUGH = 1 << 3;
        const PAGE_LEVEL_CACHE_DISABLE = 1 << 4;
    }
}

pub struct CR3;

impl CR3 {
    /// Safety
    ///
    /// Incorrect flags may violate any number of safety guarantees.
    #[inline]
    pub unsafe fn write(address: Address<Frame>, flags: CR3Flags) {
        asm!("mov cr3, {}", in(reg) (address.get().get() as u64) | flags.bits(), options(nostack));
    }

    pub fn read() -> (Address<Frame>, CR3Flags) {
        let value: u64;

        // Safety: Reading CR3 has no side effects.
        unsafe {
            asm!("mov {}, cr3", out(reg) value, options(nostack, nomem));
        }

        (
            Address::new_truncate(usize::try_from(value & !0xFFF).unwrap()),
            CR3Flags::from_bits_truncate(value),
        )
    }

    #[inline]
    pub fn refresh() {
        let value: u64;

        // Safety: Refreshing the CR3 register has no side effects (it merely purges the TLB).
        unsafe {
            asm!("mov {0}, cr3", out(reg) value, options(nostack, preserves_flags));
            asm!("mov cr3, {0}", in(reg) value, options(nostack, preserves_flags));
        }
    }
}
