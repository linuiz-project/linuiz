use core::arch::asm;
use libsys::{Address, Frame};

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CR3Flags: usize {
        const PAGE_LEVEL_WRITE_THROUGH = 1 << 3;
        const PAGE_LEVEL_CACHE_DISABLE = 1 << 4;
    }
}

pub struct CR3;

impl CR3 {
    /// # Safety
    ///
    /// Incorrect flags may violate any number of safety guarantees.
    #[inline(never)]
    pub unsafe fn write(address: Address<Frame>, flags: CR3Flags) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            asm!(
                "mov cr3, {}",
                in(reg) address.get().get() | flags.bits(),
                options(nostack, preserves_flags)
            );
        }
    }

    #[inline(always)]
    pub fn read() -> (Address<Frame>, CR3Flags) {
        let value: usize;

        // Safety: Reading CR3 has no side effects.
        unsafe {
            asm!(
                "mov {}, cr3",
                out(reg) value,
                options(nostack, nomem)
            );
        }

        (
            Address::new_truncate(value & libsys::page_mask()),
            CR3Flags::from_bits_truncate(value),
        )
    }

    #[inline]
    pub fn refresh() {
        // Safety: Refreshing the CR3 register has no side effects (it merely purges the TLB).
        unsafe {
            asm!(
                "
                mov {0}, cr3
                mov cr3, {0}
                ",
                out(reg) _,
                options(preserves_flags)
            );
        }
    }
}
