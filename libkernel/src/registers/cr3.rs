use bitflags::bitflags;
use x86_64::PhysAddr;

use crate::memory::Frame;

bitflags! {
    pub struct CR3Flags : u64 {
        const PAGE_LEVEL_WRITE_THROUGH = 1 << 3;
        const PAGE_LEVEL_CACHE_DISABLE = 1 << 4;
    }
}

pub struct CR3;

impl CR3 {
    #[inline(always)]
    pub unsafe fn write(frame: &Frame, flags: CR3Flags) {
        asm!("mov cr3, {}", in(reg) frame.addr_u64() | flags.bits(), options(nostack));
    }

    #[inline(always)]
    pub fn read() -> (Frame, CR3Flags) {
        let value: u64;

        unsafe {
            asm!("mov {}, cr3", out(reg) value, options(nostack));
        }

        (
            Frame::from_addr(PhysAddr::new(value & !0xFFF)),
            CR3Flags::from_bits_truncate(value),
        )
    }
}
