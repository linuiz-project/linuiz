use crate::memory::Frame;

bitflags::bitflags! {
    pub struct PageAttributes : usize {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER_ACCESSIBLE = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const UNCACHEABLE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        // 3 bits free for use by OS
        const NO_EXECUTE = 1 << 63;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeModify {
    Set,
    Insert,
    Remove,
    Toggle,
}

#[repr(transparent)]
pub struct PageTableEntry(usize);

impl PageTableEntry {
    pub const UNUSED: Self = Self { 0: 0 };

    pub fn set(&mut self, frame: &Frame, attributes: PageAttributes) {
        self.0 = frame.base_addr().as_usize() | attributes.bits();
    }

    pub fn get_frame(&self) -> Option<Frame> {
        if self.get_attributes().contains(PageAttributes::PRESENT) {
            Some(unsafe { Frame::from_index((self.0 & 0x000FFFFF_FFFFF000) >> 12) })
        } else {
            None
        }
    }

    pub fn set_frame(&mut self, frame: &Frame) {
        self.0 = (self.0 & PageAttributes::all().bits()) | frame.base_addr().as_usize();
    }

    pub fn get_attributes(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.0)
    }

    pub fn set_attributes(&mut self, new_attributes: PageAttributes, modify_mode: AttributeModify) {
        let mut attributes = PageAttributes::from_bits_truncate(self.0);

        match modify_mode {
            AttributeModify::Set => attributes = new_attributes,
            AttributeModify::Insert => attributes.insert(new_attributes),
            AttributeModify::Remove => attributes.remove(new_attributes),
            AttributeModify::Toggle => attributes.toggle(new_attributes),
        }

        self.0 = (self.0 & !PageAttributes::all().bits()) | attributes.bits();
    }

    pub unsafe fn set_unused(&mut self) {
        self.0 = 0;
    }
}

impl core::fmt::Debug for PageTableEntry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Page Table Entry")
            .field(&self.get_frame())
            .field(&self.get_attributes())
            .finish()
    }
}
