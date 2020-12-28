use crate::structures::memory::Frame;

const ENTRY_COUNT: usize = 512;

bitflags::bitflags! {
    pub struct EntryFlags : u64 {
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

pub struct Entry(u64);

impl Entry {
    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub fn frame(&self) -> Option<Frame> {
        if self.flags().contains(EntryFlags::PRESENT) {
            Some(Frame::new(self.0 & 0x000FFFFF_FFFFF000))
        } else {
            None
        }
    }
}

pub struct Page {
    number: usize,
}
