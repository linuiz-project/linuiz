use bitflags::bitflags;
use x86_64::PhysAddr;

use crate::memory::Frame;

bitflags! {
    pub struct CR3Flags : u64 {
        const PAGE_LEVEL_WRITETHROUGH = 1 << 3;
        const PAGE_LEVEL_CACHE_DISABLE = 1 << 4;
    }
}

pub struct CR3;

impl CR3 {
    pub unsafe fn write(frame: &Frame, flags: Option<CR3Flags>) {
        let addr = frame.addr().as_u64();
        let flags = match flags {
            Some(some) => some.bits(),
            None => 0,
        };
        let value = addr | flags;

        asm!("mov cr3, {}", in(reg) value, options(nomem, nostack));
    }

    pub fn read() -> (Frame, Option<CR3Flags>) {
        let value: u64;

        unsafe {
            asm!("mov {}, cr3", out(reg) value, options(nomem, nostack));
        }

        (
            Frame::from_addr(PhysAddr::new(value & !0xFFF)),
            CR3Flags::from_bits(value),
        )
    }
}
