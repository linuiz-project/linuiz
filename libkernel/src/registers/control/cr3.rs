use crate::{Address, Physical};
use core::arch::asm;

bitflags::bitflags! {
    pub struct CR3Flags : usize {
        const PAGE_LEVEL_WRITE_THROUGH = 1 << 3;
        const PAGE_LEVEL_CACHE_DISABLE = 1 << 4;
    }
}

pub struct CR3;

impl CR3 {
    pub unsafe fn write(addr: Address<Physical>, flags: CR3Flags) {
        debug_assert!(addr.is_frame_aligned(), "CR3 address must be frame-aligned (low 12 bits empty).");

        asm!("mov cr3, {}", in(reg) addr.as_usize() | flags.bits(), options(nostack));
    }

    pub fn read() -> (Address<Physical>, CR3Flags) {
        let value: usize;

        unsafe {
            asm!("mov {}, cr3", out(reg) value, options(nostack, nomem));
        }

        (Address::<Physical>::new(value & !CR3Flags::all().bits()), CR3Flags::from_bits_truncate(value))
    }

    #[inline(always)]
    pub fn refresh() {
        let value: usize;

        unsafe {
            asm!("mov {0}, cr3", out(reg) value, options(nostack, preserves_flags));
            asm!("mov cr3, {0}", in(reg) value, options(nostack, preserves_flags));
        }
    }
}
