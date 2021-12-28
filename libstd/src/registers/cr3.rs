use crate::{addr_ty::Physical, Address};
use core::arch::asm;

bitflags::bitflags! {
    pub struct CR3Flags : usize {
        const PAGE_LEVEL_WRITE_THROUGH = 1 << 3;
        const PAGE_LEVEL_CACHE_DISABLE = 1 << 4;
    }
}

pub struct CR3;

impl CR3 {
    pub unsafe fn write(frame: &crate::memory::Frame, flags: CR3Flags) {
        asm!("mov cr3, {}", in(reg) frame.base_addr().as_usize() | flags.bits(), options(nostack));
    }

    pub fn read() -> (Address<Physical>, CR3Flags) {
        let value: usize;

        unsafe {
            asm!("mov {}, cr3", out(reg) value, options(nostack));
        }

        (
            Address::<Physical>::new(value & !CR3Flags::all().bits()),
            CR3Flags::from_bits_truncate(value),
        )
    }

    pub fn refresh() {
        let value: usize;

        unsafe {
            asm!("mov {0}, cr3", out(reg) value, options(nostack));
            asm!("mov cr3, {0}", in(reg) value, options(nostack));
        }
    }
}
