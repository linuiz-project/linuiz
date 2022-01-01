use core::fmt;

bitflags::bitflags! {
    pub struct PageAttributes: usize {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER_ACCESSIBLE = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const UNCACHEABLE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        const MAXIMUM_ADDRESS_BIT = 1 << 48;
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
    const FRAME_BITS: usize = 0x000FFFFF_FFFFF000;
    pub const UNUSED: Self = Self(0);

    pub const fn set(&mut self, frame_index: usize, attributes: PageAttributes) {
        self.0 = (frame_index * 0x1000) | attributes.bits();
    }

    pub const fn get_frame_index(&self) -> Option<usize> {
        if self.get_attributes().contains(PageAttributes::PRESENT) {
            Some((self.0 & Self::FRAME_BITS) / 0x1000)
        } else {
            None
        }
    }

    pub const fn set_frame_index(&mut self, frame_index: usize) {
        self.0 = (self.0 & PageAttributes::all().bits()) | (frame_index * 0x1000);
    }

    // Takes this page table entry's frame, even if it is non-present.
    pub const unsafe fn take_frame_index(&mut self) -> usize {
        let frame_index = (self.0 & Self::FRAME_BITS) / 0x1000;
        self.0 &= !Self::FRAME_BITS;
        frame_index
    }

    pub const fn get_attributes(&self) -> PageAttributes {
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

    pub const unsafe fn set_unused(&mut self) {
        self.0 = 0;
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Page Table Entry")
            .field(&self.get_frame_index())
            .field(&self.get_attributes())
            .field(&format_args!("0x{:X}", self.0))
            .finish()
    }
}
