use crate::structures::memory::Frame;

bitflags::bitflags! {
    pub struct PageEntryFlags : u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER_ACCESSIBLE = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const DISABLE_CACHE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        const NO_EXECUTE = 1 << 63;
    }
}

pub struct PageEntry(u64);

impl PageEntry {
    pub fn flags(&self) -> PageEntryFlags {
        PageEntryFlags::from_bits_truncate(self.0)
    }

    pub fn frame(&self) -> Option<Frame> {
        if self.flags().contains(PageEntryFlags::PRESENT) {
            Some(Frame::new((self.0 & 0x000FFFFF_FFFFF000) as usize))
        } else {
            None
        }
    }

    pub fn set(&mut self, frame: Frame, flags: PageEntryFlags) {
        let addr_u64 = frame.address().as_u64();
        assert!((addr_u64 & !0x000FFFFF_FFFFF000000) == 0);
        self.0 = addr_u64 | flags.bits();
    }

    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    pub fn set_unused(&mut self) {
        self.0 = 0;
    }
}
