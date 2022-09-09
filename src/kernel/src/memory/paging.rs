use super::{InteriorRef, Mut, Ref};
use core::fmt;
use libkernel::{memory::Page, Address, Virtual};

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
        //  9..=10 available
        // 12..52 frame index
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
    #[cfg(target_arch = "x86_64")]
    const FRAME_INDEX_MASK: u64 = 0x000FFFFF_FFFFF000;
    #[cfg(target_arch = "riscv64")]
    const FRAME_INDEX_MASK: u64 = 0x003FFFFF_FFFFFC00;

    const FRAME_INDEX_SHIFT: u32 = Self::FRAME_INDEX_MASK.trailing_zeros();

    /// Returns an empty `Self`. All bits of this entry will be 0.
    #[inline(always)]
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn new(frame_index: usize, attributes: PageAttributes) -> Self {
        Self(((frame_index as u64) << Self::FRAME_INDEX_SHIFT) | attributes.bits())
    }

    /// Whether the page table entry is present or usable the memory controller.
    #[inline(always)]
    pub const fn is_present(&self) -> bool {
        self.get_attributes().contains(PageAttributes::PRESENT)
    }

    /// Gets the frame index of the page table entry.
    #[inline(always)]
    pub const fn get_frame_index(&self) -> usize {
        ((self.0 & Self::FRAME_INDEX_MASK) >> Self::FRAME_INDEX_SHIFT) as usize
    }

    /// Sets the entry's frame index.
    ///
    /// SAFETY: Caller must ensure changing the attributes of this entry does not cause any memory corruption side effects.
    #[inline(always)]
    pub unsafe fn set_frame_index(&mut self, frame_index: usize) {
        self.0 = (self.0 & !Self::FRAME_INDEX_MASK) | ((frame_index as u64) << Self::FRAME_INDEX_SHIFT);
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
        if !crate::arch::x64::registers::msr::IA32_EFER::get_nxe() {
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
            .field(&format_args!("{:#X}", self.get_frame_index()))
            .field(&self.get_attributes())
            .field(&format_args!("0x{:X}", self.0))
            .finish()
    }
}

pub struct PageTable<'a, RefKind: InteriorRef> {
    depth: usize,
    phys_mapped_page: Page,
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
    const fn get_depth_index(depth: usize, address: Address<Virtual>) -> usize {
        (address.as_usize() >> ((depth - 1) * 9) >> 12) & 0x1FF
    }

    pub const fn get_phys_mapped_page(&self) -> &Page {
        &self.phys_mapped_page
    }

    fn get_table_ptr(&self) -> Option<*mut PageTableEntry> {
        let root_frame_index = self.get_frame_index();
        match self.phys_mapped_page.forward_checked(root_frame_index) {
            Some(phys_mapped_table) => Some({
                // SAFETY: This type invariantly requires a valid physical mapping page, and that frame indexes be properly allocated.
                unsafe { phys_mapped_table.address().as_mut_ptr() }
            }),
            None => None,
        }
    }

    /// Gets a mutable reference to this page table's entries.
    fn get_table(&self) -> Option<&[PageTableEntry]> {
        if self.depth > 0 && self.is_present() && let Some(table_ptr) = self.get_table_ptr() {
            // SAFETY: The layout of the page table pointer is known via Intel SDM.
            Some(unsafe { core::slice::from_raw_parts(table_ptr, 512) })
        } else {
            None
        }
    }

    pub fn with_entry<T>(&self, for_page: &Page, func: impl FnOnce(&PageTableEntry) -> T) -> Option<T> {
        if let Some(entries) = self.get_table() {
            let index = Self::get_depth_index(self.depth, for_page.address());

            if self.depth > 1 {
                PageTable::<Ref> {
                    depth: self.depth - 1,
                    phys_mapped_page: self.phys_mapped_page,
                    entry: &entries[index],
                }
                .with_entry(for_page, func)
            } else {
                Some(func(&entries[index]))
            }
        } else {
            None
        }
    }
}

impl<'a> PageTable<'a, Ref> {
    /// SAFETY: Caller must ensure the provided physical mapping page and page table entry are valid.
    pub(super) unsafe fn new(depth: usize, phys_mapped_page: Page, entry: &'a PageTableEntry) -> Self {
        Self { depth, phys_mapped_page, entry }
    }
}

impl<'a> PageTable<'a, Mut> {
    /// SAFETY: Caller must ensure the provided physical mapping page and page table entry are valid.
    pub(super) unsafe fn new(depth: usize, phys_mapped_page: Page, entry: &'a mut PageTableEntry) -> Self {
        Self { depth, phys_mapped_page, entry }
    }

    /// Gets a mutable reference to this page table's entries.
    fn get_table_mut(&mut self) -> Option<&mut [PageTableEntry]> {
        if self.depth > 0 && self.is_present() {
            // SAFETY: The layout of the page table pointer is known via Intel SDM.
            Some(unsafe { core::slice::from_raw_parts_mut(self.get_table_ptr()?, 512) })
        } else {
            None
        }
    }

    pub fn with_entry_mut<T>(&mut self, for_page: &Page, func: impl FnOnce(&mut PageTableEntry) -> T) -> Option<T> {
        let depth = self.depth;
        let phys_mapped_page = self.phys_mapped_page;
        if let Some(entries) = self.get_table_mut() {
            let index = Self::get_depth_index(depth, for_page.address());
            if depth > 1 {
                PageTable::<Mut> { depth: depth - 1, phys_mapped_page: phys_mapped_page, entry: &mut entries[index] }
                    .with_entry_mut(for_page, func)
            } else {
                Some(func(&mut entries[index]))
            }
        } else {
            None
        }
    }

    /// Attempts to get a mutable reference to the page table that lies in the given entry index's frame, or
    /// creates the sub page table if it doesn't exist. This function returns `None` if it was unable to allocate
    /// a frame for the requested page table.
    pub fn with_entry_create<T>(
        &mut self,
        for_page: &Page,
        frame_manager: &'static crate::memory::FrameManager,
        func: impl FnOnce(&mut PageTableEntry) -> T,
    ) -> Option<T> {
        if !self.entry.is_present() {
            let Ok(new_frame_index) = frame_manager.lock_next() else { return None };

            // SAFETY: Entry was just created, so we know modifying it won't corrupt memory.
            unsafe {
                self.entry.set_frame_index(new_frame_index);
                self.set_attributes(PageAttributes::PTE, AttributeModify::Set);
            }

            // SAFETY: Page was just allocated, so should not contain already-borrowed memory.
            unsafe { self.phys_mapped_page.forward_checked(new_frame_index)?.clear_memory() };
        }

        let depth = self.depth;
        let phys_mapped_page = self.phys_mapped_page;
        if let Some(entries) = self.get_table_mut() {
            let index = Self::get_depth_index(depth, for_page.address());
            if depth > 1 {
                PageTable::<Mut> { depth: depth - 1, phys_mapped_page: phys_mapped_page, entry: &mut entries[index] }
                    .with_entry_create(for_page, frame_manager, func)
            } else {
                Some(func(&mut entries[index]))
            }
        } else {
            None
        }
    }
}
