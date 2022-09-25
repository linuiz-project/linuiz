use core::fmt;
use libarch::memory::{PageAttributes, PTE_FRAME_ADDRESS_MASK};
use libcommon::{
    memory::{InteriorRef, Mut, Ref},
    Address, Frame, Page, Virtual,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeModify {
    Set,
    Insert,
    Remove,
    Toggle,
}

// TODO impl table levels for attribute masking on x86
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    const FRAME_ADDRESS_SHIFT: u32 = PTE_FRAME_ADDRESS_MASK.trailing_zeros();

    /// Returns an empty `Self`. All bits of this entry will be 0.
    #[inline(always)]
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn new(frame: Address<Frame>, attributes: PageAttributes) -> Self {
        Self(((frame.index() as u64) << Self::FRAME_ADDRESS_SHIFT) | attributes.bits())
    }

    /// Whether the page table entry is present or usable the memory controller.
    #[inline(always)]
    pub const fn is_present(&self) -> bool {
        self.get_attributes().contains(PageAttributes::PRESENT)
    }

    /// Gets the frame index of the page table entry.
    #[inline(always)]
    pub const fn get_frame(&self) -> Address<Frame> {
        Address::<Frame>::new_truncate(((self.0 & PTE_FRAME_ADDRESS_MASK) >> Self::FRAME_ADDRESS_SHIFT) * 0x1000)
    }

    /// Sets the entry's frame index.
    ///
    /// SAFETY: Caller must ensure changing the attributes of this entry does not cause any memory corruption side effects.
    #[inline(always)]
    pub unsafe fn set_frame(&mut self, frame: Address<Frame>) {
        self.0 = (self.0 & !PTE_FRAME_ADDRESS_MASK) | ((frame.index() as u64) << Self::FRAME_ADDRESS_SHIFT);
    }

    /// Gets the attributes of this page table entry.
    #[inline(always)]
    pub const fn get_attributes(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.0)
    }

    /// Sets the attributes of this page table entry.
    ///
    /// SAFETY: Caller must ensure changing the attributes of this entry does not cause any memory corruption side effects.
    pub unsafe fn set_attributes(&mut self, new_attributes: PageAttributes, modify_mode: AttributeModify) {
        let mut attributes = PageAttributes::from_bits_truncate(self.0);

        match modify_mode {
            AttributeModify::Set => attributes = new_attributes,
            AttributeModify::Insert => attributes.insert(new_attributes),
            AttributeModify::Remove => attributes.remove(new_attributes),
            AttributeModify::Toggle => attributes.toggle(new_attributes),
        }

        #[cfg(target_arch = "x86_64")]
        if !libarch::x64::registers::msr::IA32_EFER::get_nxe() {
            // This bit is reserved if NXE is not supported. For now, this means silently removing it for compatability.
            attributes.remove(PageAttributes::NO_EXECUTE);
        }

        self.0 = (self.0 & !PageAttributes::all().bits()) | attributes.bits();
    }

    /// Clears the page table entry of data, setting all bits to zero.
    ///
    /// SAFETY: Caller must ensure there are no contexts which rely on the subtables this entry points to.
    #[inline]
    pub unsafe fn clear(&mut self) {
        self.0 = 0;
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Page Table Entry")
            .field(&self.get_frame())
            .field(&self.get_attributes())
            .field(&self.0)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    NotMapped,
    WalkInterrupted,
    DepthOverflow,
    NoMoreFrames,
}

pub struct PageTable<'a, RefKind: InteriorRef> {
    depth: usize,
    hhdm_address: Address<Virtual>,
    entry: <RefKind as InteriorRef>::RefType<'a, PageTableEntry>,
}

impl<RefKind: InteriorRef> core::ops::Deref for PageTable<'_, RefKind> {
    type Target = PageTableEntry;

    fn deref(&self) -> &Self::Target {
        RefKind::shared_ref(&self.entry)
    }
}

impl core::ops::DerefMut for PageTable<'_, Mut> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.entry
    }
}

impl<'a, RefKind: InteriorRef> PageTable<'a, RefKind> {
    #[inline]
    const fn get_depth_index(depth: usize, address: usize) -> usize {
        (address >> ((depth - 1) * 9) >> 12) & 0x1FF
    }

    /// Gets a mutable reference to this page table's entries.
    fn get_table(&self) -> &[PageTableEntry] {
        // SAFETY: This type's constructor requires that the physical mapped page and depth are valid values.
        let root_mapped_ptr =
            unsafe { self.hhdm_address.as_ptr::<u8>().add(self.get_frame().as_u64() as usize).cast() };
        // SAFETY: The layout of the page table pointer is known via Intel SDM.
        unsafe { core::slice::from_raw_parts(root_mapped_ptr, 512) }
    }

    pub fn with_entry<T>(
        &self,
        page: Address<Page>,
        func: impl FnOnce(Result<&PageTableEntry, PagingError>) -> T,
    ) -> T {
        let cur_depth = self.depth;
        let hhdm_address = self.hhdm_address;
        let entry = &self.get_table()[Self::get_depth_index(cur_depth, page.address().as_usize())];
        let page_depth = page.depth().unwrap_or(1);

        if cur_depth == page_depth {
            func(Ok(entry))
        } else if cur_depth > page_depth && !entry.get_attributes().contains(PageAttributes::HUGE) {
            match unsafe { PageTable::<Ref>::new(cur_depth - 1, hhdm_address, entry) } {
                Some(page_table) => page_table.with_entry(page, func),
                None => func(Err(PagingError::NotMapped)),
            }
        } else if entry.get_attributes().contains(PageAttributes::HUGE) {
            func(Err(PagingError::WalkInterrupted))
        } else {
            func(Err(PagingError::DepthOverflow))
        }
    }
}

impl<'a> PageTable<'a, Ref> {
    /// SAFETY: Caller must ensure the provided physical mapping page and page table entry are valid.
    pub(super) unsafe fn new(depth: usize, hhdm_address: Address<Virtual>, entry: &'a PageTableEntry) -> Option<Self> {
        if depth > 0 && entry.is_present() {
            Some(Self { depth, hhdm_address, entry })
        } else {
            None
        }
    }
}

impl<'a> PageTable<'a, Mut> {
    /// SAFETY: Caller must ensure the provided physical mapping page and page table entry are valid.
    pub(super) unsafe fn new(
        depth: usize,
        hhdm_address: Address<Virtual>,
        entry: &'a mut PageTableEntry,
    ) -> Option<Self> {
        if depth > 0 && entry.is_present() {
            Some(Self { depth, hhdm_address, entry })
        } else {
            None
        }
    }

    /// Gets a mutable reference to this page table's entries.
    fn get_table_mut(&mut self) -> &mut [PageTableEntry] {
        // SAFETY: This type's constructor requires that the physical mapped page and depth are valid values.
        let root_mapped_address =
            Address::<Virtual>::new_truncate(self.hhdm_address.as_u64() + self.get_frame().as_u64());
        // SAFETY: The layout of the page table pointer is known via Intel SDM.
        unsafe { core::slice::from_raw_parts_mut(root_mapped_address.as_mut_ptr(), 512) }
    }

    pub fn with_entry_mut<T>(
        &mut self,
        page: Address<Page>,
        func: impl FnOnce(Result<&mut PageTableEntry, PagingError>) -> T,
    ) -> T {
        let cur_depth = self.depth;
        let hhdm_address = self.hhdm_address;
        let entry = &mut self.get_table_mut()[Self::get_depth_index(cur_depth, page.address().as_usize())];
        let page_depth = page.depth().unwrap_or(1);

        if cur_depth == page_depth {
            func(Ok(entry))
        } else if cur_depth > page_depth && !entry.get_attributes().contains(PageAttributes::HUGE) {
            match unsafe { PageTable::<Mut>::new(cur_depth - 1, hhdm_address, entry) } {
                Some(mut page_table) => page_table.with_entry_mut(page, func),
                None => func(Err(PagingError::NotMapped)),
            }
        } else if entry.get_attributes().contains(PageAttributes::HUGE) {
            func(Err(PagingError::WalkInterrupted))
        } else {
            func(Err(PagingError::DepthOverflow))
        }
    }

    /// Attempts to get a mutable reference to the page table that lies in the given entry index's frame, or
    /// creates the sub page table if it doesn't exist. This function returns `None` if it was unable to allocate
    /// a frame for the requested page table.
    pub fn with_entry_create<T>(
        &mut self,
        page: Address<Page>,
        func: impl FnOnce(Result<&mut PageTableEntry, PagingError>) -> T,
    ) -> T {
        let cur_depth = self.depth;
        let hhdm_address = self.hhdm_address;
        let entry = &mut self.get_table_mut()[Self::get_depth_index(cur_depth, page.address().as_usize())];
        let page_depth = page.depth().unwrap_or(1);

        // TODO this doesn't handle page depth correctly for creations
        // TODO possibly handle present but no frame, or frame but no present?
        if !entry.is_present() && cur_depth > page_depth {
            let Ok(frame) = libcommon::memory::get_global_allocator().lock_next()
                else { return func(Err(PagingError::NoMoreFrames)) };
            *entry = PageTableEntry::new(frame, PageAttributes::PTE);
        }

        if cur_depth == page_depth {
            func(Ok(entry))
        } else if cur_depth > page_depth && !entry.get_attributes().contains(PageAttributes::HUGE) {
            match unsafe { PageTable::<Mut>::new(cur_depth - 1, hhdm_address, entry) } {
                Some(mut page_table) => page_table.with_entry_create(page, func),
                None => func(Err(PagingError::NotMapped)),
            }
        } else if entry.get_attributes().contains(PageAttributes::HUGE) {
            func(Err(PagingError::WalkInterrupted))
        } else {
            func(Err(PagingError::DepthOverflow))
        }
    }
}
