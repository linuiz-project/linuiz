use super::{hhdm_address, Page};
use core::{fmt, num::NonZeroU32};
use lzstd::{
    mem::{InteriorRef, Mut, Ref},
    Address, Frame, PAGE_SHIFT, PAGE_SIZE, TABLE_INDEX_SHIFT,
};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageDepth(NonZeroU32);

impl PageDepth {
    pub const MIN: Self = Self(NonZeroU32::MIN);
    pub const MAX: Self = Self({
        NonZeroU32::new({
            #[cfg(feature = "hugemem")]
            {
                5
            }
            #[cfg(not(feature = "hugemem"))]
            {
                4
            }
        })
        .unwrap()
    });

    #[inline]
    pub const fn min_align() -> usize {
        Self::MIN.align()
    }

    #[inline]
    pub const fn max_align() -> usize {
        Self::MIN.align()
    }

    #[inline]
    pub const fn new(depth: NonZeroU32) -> Self {
        Self(depth)
    }

    #[inline]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }

    #[inline]
    pub const fn align(self) -> usize {
        PAGE_SIZE.checked_shl(TABLE_INDEX_SHIFT.get() * self.0.get()).unwrap()
    }
}

#[cfg(target_arch = "x86_64")]
bitflags::bitflags! {
    #[repr(transparent)]
    pub struct PageAttributes: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const UNCACHEABLE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE = 1 << 7;
        const GLOBAL = 1 << 8;
        const DEMAND = 1 << 9;
        const NO_EXECUTE = 1 << 63;

        const RO = Self::PRESENT.bits() | Self::NO_EXECUTE.bits();
        const RW = Self::PRESENT.bits() | Self::WRITABLE.bits() | Self::NO_EXECUTE.bits();
        const RX = Self::PRESENT.bits();
        const PTE = Self::PRESENT.bits() | Self::WRITABLE.bits() | Self::USER.bits();

        const MMIO = Self::RW.bits() | Self::UNCACHEABLE.bits();
    }
}

#[cfg(target_arch = "riscv64")]
bitflags::bitflags! {
    #[repr(transparent)]
    pub struct PageAttributes: u64 {
        const VALID = 1 << 0;
        const READ = 1 << 1;
        const WRITE = 1 << 2;
        const EXECUTE = 1 << 3;
        const USER = 1 << 4;
        const GLOBAL = 1 << 5;
        const ACCESSED = 1 << 6;
        const DIRTY = 1 << 7;

        const RO = Self::VALID.bits() | Self::READ.bits();
        const RW = Self::VALID.bits() | Self::READ.bits() | Self::WRITE.bits();
        const RX = Self::VALID.bits() | Self::READ.bits() | Self::EXECUTE.bits();
        const PTE = Self::VALID.bits() | Self::READ.bits() | Self::WRITE.bits();

        const MMIO = Self::RW.bits();
    }
}

#[cfg(target_arch = "x86_64")]
pub const PTE_FRAME_ADDRESS_MASK: u64 = 0x000FFFFF_FFFFF000;
#[cfg(target_arch = "riscv64")]
pub const PTE_FRAME_ADDRESS_MASK: u64 = 0x003FFFFF_FFFFFC00;

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
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn new(frame: Address<Frame>, attributes: PageAttributes) -> Self {
        Self(((frame.index() as u64) << Self::FRAME_ADDRESS_SHIFT) | attributes.bits())
    }

    /// Whether the page table entry is present or usable the memory controller.
    #[inline]
    pub const fn is_present(&self) -> bool {
        self.get_attributes().contains(PageAttributes::PRESENT)
    }

    /// Gets the frame index of the page table entry.
    #[inline]
    pub fn get_frame(&self) -> Address<Frame> {
        Address::new_truncate((self.0 & PTE_FRAME_ADDRESS_MASK) as usize)
    }

    /// Sets the entry's frame index.
    ///
    /// ### Safety
    ///
    /// Caller must ensure changing the attributes of this entry does not cause any memory corruption side effects.
    #[inline]
    pub unsafe fn set_frame(&mut self, frame: Address<Frame>) {
        self.0 = (self.0 & !PTE_FRAME_ADDRESS_MASK) | ((frame.index() as u64) << Self::FRAME_ADDRESS_SHIFT);
    }

    /// Gets the attributes of this page table entry.
    #[inline]
    pub const fn get_attributes(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.0)
    }

    /// Sets the attributes of this page table entry.
    ///
    /// ### Safety
    ///
    /// Caller must ensure changing the attributes of this entry does not cause any memory corruption side effects.
    pub unsafe fn set_attributes(&mut self, new_attributes: PageAttributes, modify_mode: AttributeModify) {
        let mut attributes = PageAttributes::from_bits_truncate(self.0);

        match modify_mode {
            AttributeModify::Set => attributes = new_attributes,
            AttributeModify::Insert => attributes.insert(new_attributes),
            AttributeModify::Remove => attributes.remove(new_attributes),
            AttributeModify::Toggle => attributes.toggle(new_attributes),
        }

        #[cfg(target_arch = "x86_64")]
        if !crate::arch::x64::registers::msr::IA32_EFER::get_nxe() {
            // This bit is reserved if NXE is not supported. For now, this means silently removing it for compatability.
            attributes.remove(PageAttributes::NO_EXECUTE);
        }

        self.0 = (self.0 & !PageAttributes::all().bits()) | attributes.bits();
    }

    /// Clears the page table entry of data, setting all bits to zero.
    ///
    /// ### Safety
    ///
    /// Caller must ensure there are no contexts which rely on the subtables this entry points to.
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
    DepthUnderflow,
    NoMoreFrames,
    Unknown,
}

pub struct PageTable<'a, RefKind: InteriorRef> {
    depth: PageDepth,
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

impl<RefKind: InteriorRef> PageTable<'_, RefKind> {
    #[inline]
    pub const fn depth(&self) -> PageDepth {
        self.depth
    }

    #[inline]
    pub const fn next_depth(&self) -> Option<PageDepth> {
        NonZeroU32::new(self.depth().get().get() - 1).map(PageDepth::new)
    }

    /// # Safety
    ///
    /// Returned pointer must not be used mutably in immutable `&self` contexts.
    unsafe fn get_entry_ptr(&self, page: Address<Page>) -> *mut PageTableEntry {
        // Safety: Type requires that the internal entry has a valid frame.
        let table_ptr = unsafe { hhdm_address().as_ptr().add(self.get_frame().get()) };
        let entry_index = {
            let index_shift = (self.depth().get().get() - 1) * TABLE_INDEX_SHIFT.get();
            let index_mask = (1 << TABLE_INDEX_SHIFT.get()) - 1;

            (page.get() >> index_shift >> PAGE_SHIFT.get()) & index_mask
        };
        // Safety: `entry_index` guarantees a value that does not exceed the table size.
        unsafe { table_ptr.cast::<PageTableEntry>().add(entry_index) }
    }

    fn get_entry(&self, page: Address<Page>) -> &PageTableEntry {
        // Safety: The provided invariants being met, this will dereference a valid pointer for the type.
        unsafe { self.get_entry_ptr(page).as_ref().unwrap() }
    }
}

impl<'a> PageTable<'a, Ref> {
    /// # Safety
    ///
    /// Caller must ensure the provided physical mapping page and page table entry are valid.
    pub(super) unsafe fn new(depth: PageDepth, entry: &'a PageTableEntry) -> Option<Self> {
        if entry.is_present() {
            Some(Self { depth, entry })
        } else {
            None
        }
    }

    pub fn with_entry<T>(
        &self,
        page: Address<Page>,
        to_depth: Option<PageDepth>,
        with_fn: impl FnOnce(Result<&PageTableEntry, PagingError>) -> T,
    ) -> T {
        let entry = self.get_entry(page);
        let is_huge = entry.get_attributes().contains(PageAttributes::HUGE);

        match to_depth {
            Some(to_depth) if self.depth() == to_depth => with_fn(Ok(entry)),
            Some(to_depth) if self.depth() > to_depth => {
                match is_huge {
                    false if let Some(next_depth) = self.next_depth() => {
                        // Safety: If the page table entry is present, then it's a valid entry, all bits accounted.
                        match unsafe { PageTable::<Ref>::new(next_depth, entry) } {
                            Some( page_table) => page_table.with_entry(page, Some(to_depth), with_fn),
                            None => with_fn(Err(PagingError::NotMapped)),
                        }
                    }

                    true => with_fn(Err(PagingError::WalkInterrupted)),
                    false => with_fn(Err(PagingError::DepthUnderflow)),
                }
            }

            None if is_huge => with_fn(Ok(entry)),

            _ => with_fn(Err(PagingError::Unknown)),
        }
    }
}

impl<'a> PageTable<'a, Mut> {
    /// # Safety
    ///
    /// Caller must ensure the provided physical mapping page and page table entry are valid.
    pub(super) unsafe fn new(depth: PageDepth, entry: &'a mut PageTableEntry) -> Option<Self> {
        if entry.is_present() {
            Some(Self { depth, entry })
        } else {
            None
        }
    }

    fn get_entry_mut(&self, page: Address<Page>) -> &mut PageTableEntry {
        // Safety: The provided invariants being met, this will dereference a valid pointer for the type.
        unsafe { self.get_entry_ptr(page).as_mut().unwrap() }
    }

    pub fn with_entry_mut<T>(
        &mut self,
        page: Address<Page>,
        to_depth: Option<PageDepth>,
        with_fn: impl FnOnce(Result<&mut PageTableEntry, PagingError>) -> T,
    ) -> T {
        let entry = self.get_entry_mut(page);
        let is_huge = entry.get_attributes().contains(PageAttributes::HUGE);

        match to_depth {
            Some(to_depth) if self.depth() == to_depth => with_fn(Ok(entry)),
            Some(to_depth) if self.depth() > to_depth => {
                match is_huge {
                    false if let Some(next_depth) = self.next_depth() => {
                        // Safety: If the page table entry is present, then it's a valid entry, all bits accounted.
                        match unsafe { PageTable::<Mut>::new(next_depth, entry) } {
                            Some(mut page_table) => page_table.with_entry_mut(page, Some(to_depth), with_fn),
                            None => with_fn(Err(PagingError::NotMapped)),
                        }
                    }

                    true => with_fn(Err(PagingError::WalkInterrupted)),
                    false => with_fn(Err(PagingError::DepthUnderflow)),
                }
            }

            None if is_huge => with_fn(Ok(entry)),

            _ => with_fn(Err(PagingError::Unknown)),
        }
    }

    /// Attempts to get a mutable reference to the page table that lies in the given entry index's frame, or
    /// creates the sub page table if it doesn't exist. This function returns `None` if it was unable to allocate
    /// a frame for the requested page table.
    pub fn with_entry_create<T>(
        &mut self,
        page: Address<Page>,
        to_depth: PageDepth,
        with_fn: impl FnOnce(Result<&mut PageTableEntry, PagingError>) -> T,
    ) -> T {
        let entry = self.get_entry_mut(page);
        let is_huge = entry.get_attributes().contains(PageAttributes::HUGE);

        // TODO this doesn't handle page depth correctly for creations
        // TODO possibly handle present but no frame, or frame but no present?
        if !entry.is_present() && self.depth() > to_depth {
            let Ok(frame) = crate::memory::PMM.next_frame()
                    else { return with_fn(Err(PagingError::NoMoreFrames)) };
            *entry = PageTableEntry::new(frame, PageAttributes::PTE);
        }

        match to_depth {
            to_depth if self.depth() == to_depth => with_fn(Ok(entry)),
            to_depth if self.depth() > to_depth => {
                match is_huge {
                    false if let Some(next_depth) = self.next_depth() => {
                        // Safety: If the page table entry is present, then it's a valid entry, all bits accounted.
                        match unsafe { PageTable::<Mut>::new(next_depth, entry) } {
                            Some(mut page_table) => page_table.with_entry_create(page, to_depth, with_fn),
                            None => with_fn(Err(PagingError::NotMapped)),
                        }
                    }

                    true => with_fn(Err(PagingError::WalkInterrupted)),
                    false => with_fn(Err(PagingError::DepthUnderflow)),
                }
            }

            _ => with_fn(Err(PagingError::Unknown)),
        }
    }
}
