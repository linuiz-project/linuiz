use bitflags::bitflags;
use x86_64::PhysAddr;

use crate::structures::memory::Frame;

bitflags! {
    pub struct PageAttributes : u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER_ACCESSIBLE = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const DISABLE_CACHE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        // 3 bits free for use by OS
        const NO_EXECUTE = 1 << 63;
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct PageDescriptor(u64);

impl PageDescriptor {
    pub const fn unused() -> Self {
        Self { 0: 0 }
    }

    pub fn attribs(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.0)
    }

    pub fn frame(&self) -> Option<Frame> {
        if self.attribs().contains(PageAttributes::PRESENT) {
            Some(Frame::new(Frame::trim_addr(self.0)))
        } else {
            None
        }
    }

    pub fn set(&mut self, frame: &Frame, attribs: PageAttributes) {
        let addr = frame.addr().as_u64();
        assert!((addr & !0x000FFFFF_FFFFF000000) == 0);
        self.0 = addr | attribs.bits();
    }

    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    pub fn set_unused(&mut self) {
        self.0 = 0;
    }
}
