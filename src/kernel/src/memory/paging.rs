use crate::memory::{hhdm_address, PageDepth};
use core::{cmp::Ordering, fmt, ptr::NonNull};
use libsys::{
    mem::{InteriorRef, Mut, Ref},
    page_shift, page_size, table_index_mask, table_index_shift, table_index_size, Address, Frame, Page, Virtual,
};

#[derive(Debug)]
pub enum Error {
    /// The underlying allocator is out of memory.
    AllocError,

    /// Unexpected huge page was encountered.
    HugePage,

    /// The specified page is not mapped.
    NotMapped(Address<Virtual>),
}

impl core::error::Error for Error {}

crate::default_display_impl!(Error);
crate::err_result_type!(Error);

#[cfg(target_arch = "x86_64")]
bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Attributes : u64 {
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
    pub struct Attributes: u64 {
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
pub struct TableEntry(u64);

impl TableEntry {
    const FRAME_ADDRESS_SHIFT: u32 = PTE_FRAME_ADDRESS_MASK.trailing_zeros();

    /// Returns an empty `Self`. All bits of this entry will be 0.
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn new(frame: Address<Frame>, attributes: Attributes) -> Self {
        Self(((frame.index() as u64) << Self::FRAME_ADDRESS_SHIFT) | attributes.bits())
    }

    /// Sets the entry's data.
    ///
    /// Safety
    ///
    /// Caller must ensure changing the attributes of this entry does not cause memory corruption.
    pub unsafe fn set(&mut self, frame: Address<Frame>, attributes: Attributes) {
        self.0 = ((frame.index() as u64) << Self::FRAME_ADDRESS_SHIFT) | attributes.bits();
    }

    /// Gets the frame index of the page table entry.
    #[inline]
    pub fn get_frame(&self) -> Address<Frame> {
        Address::new_truncate((self.0 & PTE_FRAME_ADDRESS_MASK) as usize)
    }

    /// Sets the entry's frame index.
    ///
    /// Safety
    ///
    /// Caller must ensure changing the attributes of this entry does not cause memory corruption.
    #[inline]
    pub unsafe fn set_frame(&mut self, frame: Address<Frame>) {
        self.0 = (self.0 & !PTE_FRAME_ADDRESS_MASK) | ((frame.index() as u64) << Self::FRAME_ADDRESS_SHIFT);
    }

    /// Gets the attributes of this page table entry.
    #[inline]
    pub const fn get_attributes(&self) -> Attributes {
        Attributes::from_bits_truncate(self.0)
    }

    /// Sets the attributes of this page table entry.
    ///
    /// Safety
    ///
    /// Caller must ensure changing the attributes of this entry does not cause any memory corruption side effects.
    pub unsafe fn set_attributes(&mut self, new_attributes: Attributes, modify_mode: AttributeModify) {
        let mut attributes = Attributes::from_bits_truncate(self.0);

        match modify_mode {
            AttributeModify::Set => attributes = new_attributes,
            AttributeModify::Insert => attributes.insert(new_attributes),
            AttributeModify::Remove => attributes.remove(new_attributes),
            AttributeModify::Toggle => attributes.toggle(new_attributes),
        }

        #[cfg(target_arch = "x86_64")]
        if !crate::arch::x64::registers::msr::IA32_EFER::get_nxe() {
            // This bit is reserved if NXE is not supported. For now, this means silently removing it for compatability.
            attributes.remove(Attributes::NO_EXECUTE);
        }

        self.0 = (self.0 & !Attributes::all().bits()) | attributes.bits();
    }

    #[inline]
    pub const fn is_present(&self) -> bool {
        self.get_attributes().contains(Attributes::PRESENT)
    }

    #[inline]
    pub const fn is_huge(&self) -> bool {
        self.get_attributes().contains(Attributes::HUGE)
    }

    /// Clears the page table entry of data, setting all bits to zero.
    ///
    /// Safety
    ///
    /// Caller must ensure there are no contexts which rely on the subtables this entry points to.
    #[inline]
    pub unsafe fn clear(&mut self) {
        self.0 = 0;
    }
}

impl fmt::Debug for TableEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Page Table Entry")
            .field(&self.get_frame())
            .field(&self.get_attributes())
            .field(&self.0)
            .finish()
    }
}

pub struct TableEntryCell<'a, RefKind: InteriorRef> {
    depth: PageDepth,
    entry: <RefKind as InteriorRef>::RefType<'a, TableEntry>,
}

impl<RefKind: InteriorRef> core::ops::Deref for TableEntryCell<'_, RefKind> {
    type Target = TableEntry;

    fn deref(&self) -> &Self::Target {
        RefKind::shared_ref(&self.entry)
    }
}

impl core::ops::DerefMut for TableEntryCell<'_, Mut> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.entry
    }
}

impl<RefKind: InteriorRef> TableEntryCell<'_, RefKind> {
    #[inline]
    pub const fn depth(&self) -> PageDepth {
        self.depth
    }

    /// # Safety
    ///
    /// Returned pointer must not be used mutably in immutable `&self` contexts.
    unsafe fn get_sub_entry_ptr(&self, page: Address<Page>) -> NonNull<TableEntry> {
        let entry_index = {
            let index_shift = (self.depth().get() - 1) * table_index_shift().get();
            (page.get().get() >> index_shift >> page_shift().get()) & table_index_mask()
        };

        debug_assert!(entry_index < table_index_size().get(), "entry index exceeds maximum");

        // Safety: Type requires that the internal entry has a valid frame.
        let table_ptr = unsafe { hhdm_address().as_ptr().add(self.get_frame().get().get()).cast::<TableEntry>() };
        // Safety: `entry_index` guarantees a value that does not exceed the table size.
        let entry_ptr = unsafe { table_ptr.add(entry_index) };

        debug_assert!(entry_ptr.is_aligned_to(core::mem::align_of::<TableEntry>()));

        NonNull::new(entry_ptr).unwrap()
    }

    fn get(&self, page: Address<Page>) -> &TableEntry {
        // Safety:
        //  - Pointer is not used mutably.
        //  - Reference is guaranteed by the function to be properly aligned.
        //  - `PageTableEntry` has no uninitialized states, so is valid for any bit sequence.
        unsafe { self.get_sub_entry_ptr(page).as_ref() }
    }
}

impl<'a> TableEntryCell<'a, Ref> {
    /// Safety
    ///
    /// - Page table entry must point to a valid page table.
    /// - Page table depth must be correct for the provided table.
    pub(super) unsafe fn new(depth: PageDepth, entry: &'a TableEntry) -> Self {
        Self { depth, entry }
    }

    pub fn with_entry<T>(
        &self,
        page: Address<Page>,
        to_depth: Option<PageDepth>,
        with_fn: impl FnOnce(&TableEntry) -> T,
    ) -> Result<T> {
        match (self.depth().cmp(&to_depth.unwrap_or(PageDepth::min())), self.is_huge(), self.depth().next()) {
            (Ordering::Equal, _, _) => Ok(with_fn(self.entry)),
            (Ordering::Greater, false, Some(next_depth)) => {
                let sub_entry = self.get(page);

                if !sub_entry.is_present() {
                    Err(Error::NotMapped(page.get()))
                } else {
                    unsafe { TableEntryCell::<Ref>::new(next_depth, sub_entry) }.with_entry(page, to_depth, with_fn)
                }
            }

            (Ordering::Greater, true, _) => Err(Error::HugePage),
            _ => panic!("page table walk in expected state"),
        }
    }
}

impl<'a> TableEntryCell<'a, Mut> {
    /// Safety
    ///
    /// - Page table entry must point to a valid page table.
    /// - Page table depth must be correct for the provided table.
    pub(super) unsafe fn new(depth: PageDepth, entry: &'a mut TableEntry) -> Self {
        Self { depth, entry }
    }

    fn get_mut(&self, page: Address<Page>) -> &mut TableEntry {
        // Safety:
        //  - Pointer is used mutably in an `&mut self` context.
        //  - Reference is guaranteed by the function to be properly aligned.
        //  - `PageTableEntry` has no uninitialized states, so is valid for any bit sequence.
        unsafe { self.get_sub_entry_ptr(page).as_mut() }
    }

    pub fn with_entry_mut<T>(
        &mut self,
        page: Address<Page>,
        to_depth: Option<PageDepth>,
        with_fn: impl FnOnce(&mut TableEntry) -> T,
    ) -> Result<T> {
        match (self.depth().cmp(&to_depth.unwrap_or(PageDepth::min())), self.is_huge(), self.depth().next()) {
            (Ordering::Equal, _, _) => Ok(with_fn(self.entry)),
            (Ordering::Greater, false, Some(next_depth)) => {
                let sub_entry = self.get_mut(page);

                if !sub_entry.is_present() {
                    Err(Error::NotMapped(page.get()))
                } else {
                    unsafe { TableEntryCell::<Mut>::new(next_depth, sub_entry) }.with_entry_mut(page, to_depth, with_fn)
                }
            }

            (Ordering::Greater, true, _) => Err(Error::HugePage),
            _ => panic!("page table walk in expected state"),
        }
    }

    /// Attempts to get a mutable reference to the page table that lies in the given entry index's frame, or
    /// creates the sub page table if it doesn't exist. This function returns `None` if it was unable to allocate
    /// a frame for the requested page table.
    pub fn with_entry_create<T>(
        &mut self,
        page: Address<Page>,
        to_depth: PageDepth,
        with_fn: impl FnOnce(&mut TableEntry) -> T,
    ) -> Result<T> {
        match (self.depth().cmp(&to_depth), self.is_huge(), self.depth().next()) {
            (Ordering::Equal, _, _) => Ok(with_fn(self.entry)),

            (Ordering::Greater, false, Some(next_depth)) => {
                if !self.is_present() {
                    debug_assert!(
                        self.get_frame() == Address::default(),
                        "page table entry is non-present, but has a present frame address"
                    );

                    let Ok(frame) = crate::memory::PMM.next_frame()
                    else {
                        return Err(Error::AllocError)
                    };

                    // Clear the frame to avoid corrupted PTEs.
                    // Safety: Frame was just allocated, and so is unused outside this context.
                    unsafe {
                        core::ptr::write_bytes(hhdm_address().as_ptr().add(frame.get().get()), 0x0, page_size().get());
                    }

                    // Set the entry frame and set attributes to make a valid PTE.
                    // Safety: Entry currently points to no memory.
                    unsafe {
                        self.set(frame, Attributes::PTE);
                    }
                }

                // Safety: If the page table entry is present, then it's a valid entry, all bits accounted.
                unsafe {
                    TableEntryCell::<Mut>::new(next_depth, self.get_mut(page))
                        .with_entry_create(page, to_depth, with_fn)
                }
            }

            (Ordering::Greater, true, _) => Err(Error::HugePage),
            _ => panic!("page table walk in expected state"),
        }
    }
}
