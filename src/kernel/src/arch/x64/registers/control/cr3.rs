use core::arch::asm;
use lzstd::{Address, Frame};

bitflags::bitflags! {
    pub struct CR3Flags : usize {
        const PAGE_LEVEL_WRITE_THROUGH = 1 << 3;
        const PAGE_LEVEL_CACHE_DISABLE = 1 << 4;
    }
}

pub struct CR3;

impl CR3 {
    pub unsafe fn write(address: Address<Frame>, flags: CR3Flags) {
        asm!("mov cr3, {}", in(reg) address.get() | flags.bits(), options(nostack));
    }

    pub fn read() -> (Address<Frame>, CR3Flags) {
        let value: usize;

        unsafe {
            asm!("mov {}, cr3", out(reg) value, options(nostack, nomem));
        }

        (Address::new_truncate(value & !CR3Flags::all().bits()), CR3Flags::from_bits_truncate(value))
    }

    #[inline]
    pub fn refresh() {
        let value: usize;

        unsafe {
            asm!("mov {0}, cr3", out(reg) value, options(nostack, preserves_flags));
            asm!("mov cr3, {0}", in(reg) value, options(nostack, preserves_flags));
        }
    }
}
