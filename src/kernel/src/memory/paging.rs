use super::hhdm_address;
use core::fmt;
use libsys::{
    mem::{InteriorRef, Mut, Ref},
    page_shift, page_size, table_index_shift, Address, Frame, Page,
};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Eq, Ord)]
pub struct PageDepth(u32);

impl const PartialEq for PageDepth {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl const PartialOrd for PageDepth {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl PageDepth {
    pub const MIN: Self = Self(u32::MIN);
    pub const MAX: Self = Self({
        #[cfg(feature = "hugemem")]
        {
            5
        }

        #[cfg(not(feature = "hugemem"))]
        {
            4
        }
    });

    pub fn current() -> Self {
        Self(crate::memory::current_paging_levels())
    }

    #[inline]
    pub const fn min_align() -> usize {
        Self::MIN.align()
    }

    #[inline]
    pub const fn max_align() -> usize {
        Self::MIN.align()
    }

    #[inline]
    pub const fn new(depth: u32) -> Self {
        Self(depth)
    }

    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn align(self) -> usize {
        page_size().get().checked_shl(table_index_shift().get() * self.get()).unwrap()
    }

    #[inline]
    pub const fn next(self) -> Option<Self> {
        self.get().checked_sub(1).map(PageDepth::new)
    }

    #[inline]
    pub const fn is_min(self) -> bool {
        self == Self::MIN
    }

    #[inline]
    pub const fn is_max(self) -> bool {
        self == Self::MAX
    }
}

#[cfg(target_arch = "x86_64")]
bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PageAttributes : u64 {
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

    pub fn set(&mut self, frame: Address<Frame>, attributes: PageAttributes) {
        self.0 = ((frame.index() as u64) << Self::FRAME_ADDRESS_SHIFT) | attributes.bits();
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

    #[inline]
    pub const fn is_present(&self) -> bool {
        self.get_attributes().contains(PageAttributes::PRESENT)
    }

    #[inline]
    pub const fn is_huge(&self) -> bool {
        self.get_attributes().contains(PageAttributes::HUGE)
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

pub struct PageTableEntryCell<'a, RefKind: InteriorRef> {
    depth: PageDepth,
    entry: <RefKind as InteriorRef>::RefType<'a, PageTableEntry>,
}

impl<RefKind: InteriorRef> core::ops::Deref for PageTableEntryCell<'_, RefKind> {
    type Target = PageTableEntry;

    fn deref(&self) -> &Self::Target {
        RefKind::shared_ref(&self.entry)
    }
}

impl core::ops::DerefMut for PageTableEntryCell<'_, Mut> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.entry
    }
}

impl<RefKind: InteriorRef> PageTableEntryCell<'_, RefKind> {
    #[inline]
    pub const fn depth(&self) -> PageDepth {
        self.depth
    }

    /// # Safety
    ///
    /// Returned pointer must not be used mutably in immutable `&self` contexts.
    unsafe fn get_sub_entry_ptr(&self, page: Address<Page>) -> *mut PageTableEntry {
        // Safety: Type requires that the internal entry has a valid frame.
        let table_ptr = unsafe { hhdm_address().as_ptr().add(self.get_frame().get().get()) };
        let entry_index = {
            let index_shift = (self.depth().get().checked_sub(1).unwrap()) * table_index_shift().get();
            let index_mask = (1 << table_index_shift().get()) - 1;

            (page.get().get() >> index_shift >> page_shift().get()) & index_mask
        };
        // Safety: `entry_index` guarantees a value that does not exceed the table size.
        unsafe { table_ptr.cast::<PageTableEntry>().add(entry_index) }
    }

    fn get(&self, page: Address<Page>) -> &PageTableEntry {
        // Safety: The provided invariants being met, this will dereference a valid pointer for the type.
        unsafe { self.get_sub_entry_ptr(page).as_ref().unwrap() }
    }
}

impl<'a> PageTableEntryCell<'a, Ref> {
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
        with_fn: impl FnOnce(&PageTableEntry) -> T,
    ) -> Result<T, PagingError> {
        match (self.depth() == to_depth.unwrap_or(PageDepth::MIN), self.is_huge(), self.depth().next()) {
            (true, _, _) => Ok(with_fn(self.entry)),

            (false, true, _) => Err(PagingError::WalkInterrupted),
            (false, false, None) => Err(PagingError::DepthUnderflow),

            (false, false, Some(next_depth)) => {
                let sub_entry = self.get(page);

                if !sub_entry.is_present() {
                    Err(PagingError::NotMapped)
                } else {
                    match unsafe { PageTableEntryCell::<Ref>::new(next_depth, sub_entry) } {
                        Some(sub_entry_cell) => sub_entry_cell.with_entry(page, to_depth, with_fn),
                        None => Err(PagingError::NotMapped),
                    }
                }
            }
        }
    }
}

impl<'a> PageTableEntryCell<'a, Mut> {
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

    fn get_mut(&self, page: Address<Page>) -> &mut PageTableEntry {
        // Safety: The provided invariants being met, this will dereference a valid pointer for the type.
        unsafe { self.get_sub_entry_ptr(page).as_mut().unwrap() }
    }

    pub fn with_entry_mut<T>(
        &mut self,
        page: Address<Page>,
        to_depth: Option<PageDepth>,
        with_fn: impl FnOnce(&mut PageTableEntry) -> T,
    ) -> Result<T, PagingError> {
        match (self.depth() == to_depth.unwrap_or(PageDepth::MIN), self.is_huge(), self.depth().next()) {
            (true, _, _) => Ok(with_fn(self.entry)),

            (false, true, _) => Err(PagingError::WalkInterrupted),
            (false, false, None) => Err(PagingError::DepthUnderflow),

            (false, false, Some(next_depth)) => {
                let sub_entry = self.get_mut(page);

                if !sub_entry.is_present() {
                    Err(PagingError::NotMapped)
                } else {
                    match unsafe { PageTableEntryCell::<Mut>::new(next_depth, sub_entry) } {
                        Some(mut sub_entry_cell) => sub_entry_cell.with_entry_mut(page, to_depth, with_fn),
                        None => Err(PagingError::NotMapped),
                    }
                }
            }
        }
    }

    /// Attempts to get a mutable reference to the page table that lies in the given entry index's frame, or
    /// creates the sub page table if it doesn't exist. This function returns `None` if it was unable to allocate
    /// a frame for the requested page table.
    pub fn with_entry_create<T>(
        &mut self,
        page: Address<Page>,
        to_depth: PageDepth,
        with_fn: impl FnOnce(&mut PageTableEntry) -> T,
    ) -> Result<T, PagingError> {
        let entry = self.get_mut(page);

        if !entry.is_present() {
            debug_assert!(
                entry.get_frame() == Address::default(),
                "page table entry is non-present, but has a present frame address"
            );

            let Ok(frame) = crate::memory::PMM.next_frame()
            else {
                return Err(PagingError::NoMoreFrames)
            };

            entry.set(frame, PageAttributes::PTE);
        }

        match (self.depth().cmp(&to_depth), self.depth().next()) {
            (core::cmp::Ordering::Equal, _) => Ok(with_fn(entry)),

            (core::cmp::Ordering::Greater, _) if entry.is_huge() => Err(PagingError::WalkInterrupted),
            (core::cmp::Ordering::Greater, Some(next_depth)) => {
                // Safety: If the page table entry is present, then it's a valid entry, all bits accounted.
                match unsafe { PageTableEntryCell::<Mut>::new(next_depth, entry) } {
                    Some(mut page_table) => page_table.with_entry_create(page, to_depth, with_fn),
                    None => Err(PagingError::NotMapped),
                }
            }

            _ => Err(PagingError::DepthUnderflow),
        }
    }
}
