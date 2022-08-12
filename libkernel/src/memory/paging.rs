use crate::{Address, Virtual};
use core::fmt;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page {
    index: usize,
}

impl Page {
    pub const fn null() -> Self {
        Self { index: 0 }
    }

    pub const fn from_index(index: usize) -> Self {
        Self { index }
    }

    pub const fn from_addr(addr: Address<Virtual>) -> Self {
        if addr.is_aligned_to(0x1000) {
            Self { index: addr.page_index() }
        } else {
            panic!("page address is not page-aligned")
        }
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        let ptr_usize = ptr as usize;

        assert_eq!(ptr_usize % 0x1000, 0, "Pointers must be page-aligned to use as page addresses.");

        Self { index: ptr_usize / 0x1000 }
    }

    pub const fn containing_addr(addr: Address<Virtual>) -> Self {
        Self { index: addr.page_index() }
    }

    pub const fn range(start: usize, end: usize) -> core::ops::Range<Self> {
        Self::from_index(start)..Self::from_index(end)
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn base_addr(&self) -> Address<Virtual> {
        unsafe { crate::Address::new_unsafe(self.index * 0x1000) }
    }

    pub const fn as_ptr<T>(&self) -> *const T {
        (self.index * 0x1000) as *const T
    }

    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        (self.index * 0x1000) as *mut T
    }

    pub unsafe fn mem_clear(&mut self) {
        core::ptr::write_bytes::<usize>(self.as_mut_ptr(), 0x0, 0x1000 / core::mem::size_of::<usize>());
    }

    pub fn to(&self, count: usize) -> Option<PageIterator> {
        self.forward_checked(count).map(|end| PageIterator::new(self, &end))
    }

    pub fn forward_checked(&self, count: usize) -> Option<Self> {
        self.index().checked_add(count).map(|new_index| Self::from_index(new_index))
    }

    pub fn backward_checked(&self, count: usize) -> Option<Self> {
        self.index().checked_sub(count).map(|new_index| Self::from_index(new_index))
    }

    /// Clears the 4KiB region from this page's start to its end.
    pub unsafe fn clear_memory(&self) {
        core::ptr::write_bytes(self.as_mut_ptr::<u8>(), 0, 0x1000);
    }
}

impl core::iter::Step for Page {
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start.forward_checked(count)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start.backward_checked(count)
    }

    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        Some(end.index() - start.index())
    }
}

impl core::fmt::Debug for Page {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Page").field(&format_args!("0x{:X}", self.index << 12)).finish()
    }
}

pub struct PageIterator {
    start: Page,
    current: Page,
    end: Page,
}

impl PageIterator {
    pub fn new(start: &Page, end: &Page) -> Self {
        Self { start: *start, current: *start, end: *end }
    }

    pub fn start(&self) -> &Page {
        &self.start
    }

    pub fn current(&self) -> &Page {
        &self.current
    }

    pub fn end(&self) -> &Page {
        &self.end
    }

    pub fn captured_len(&self) -> usize {
        self.end().index() - self.start().index()
    }

    pub fn reset(&mut self) {
        self.current = *self.start();
    }
}

impl Iterator for PageIterator {
    type Item = Page;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.index < self.end.index {
            let page = self.current.clone();
            self.current.index += 1;
            Some(page)
        } else {
            None
        }
    }
}

impl ExactSizeIterator for PageIterator {
    fn len(&self) -> usize {
        self.end().index() - self.start().index()
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct PageAttributes: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USERSPACE = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const UNCACHEABLE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        // We don't support huge pages for now.
        // const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        //  9..=11 available
        // 12..52 frame index
        // 52..=58 available
        const NO_EXECUTE = 1 << 63;

        const RO = Self::PRESENT.bits() | Self::NO_EXECUTE.bits();
        const RW = Self::PRESENT.bits() | Self::WRITABLE.bits() | Self::NO_EXECUTE.bits();
        const RX = Self::PRESENT.bits();
        const MMIO = Self::RW.bits() | Self::UNCACHEABLE.bits();
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
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    #[cfg(target_arch = "x86_64")]
    const FRAME_INDEX_MASK: u64 = 0x000FFFFF_FFFFF000;

    /// Returns an empty `Self`. All bits of this entry will be 0.
    #[inline(always)]
    pub const fn empty() -> Self {
        Self(0)

        // Ensure RISC-V 64 PBMT bits are 0 if unsupported
    }

    /// Sets the frame and attributes of this entry.
    #[inline(always)]
    pub const fn set(&mut self, frame_index: usize, attributes: PageAttributes) {
        #[cfg(target_arch = "x86_64")]
        {
            self.0 = ((frame_index as u64) * 0x1000) | attributes.bits();
        }
    }

    /// Whether the page table entry is present or usable the memory controller.
    #[inline(always)]
    pub const fn is_present(&self) -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            self.get_attributes().contains(PageAttributes::PRESENT)
        }
    }

    /// Gets the frame index of the page table entry.
    #[inline(always)]
    pub const fn get_frame_index(&self) -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            ((self.0 & Self::FRAME_INDEX_MASK) / 0x1000) as usize
        }
    }

    // Takes this page table entry's frame, even if it is non-present.
    pub const unsafe fn take_frame_index(&mut self) -> usize {
        let frame_index = self.get_frame_index();
        self.0 &= !Self::FRAME_INDEX_MASK;
        frame_index
    }

    /// Gets the attributes of this page table entry.
    #[inline(always)]
    pub const fn get_attributes(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.0)
    }

    /// Sets the attributes of this page table entry.
    pub fn set_attributes(&mut self, new_attribs: PageAttributes, modify_mode: AttributeModify) {
        let mut attributes = PageAttributes::from_bits_truncate(self.0);

        match modify_mode {
            AttributeModify::Set => attributes = new_attribs,
            AttributeModify::Insert => attributes.insert(new_attribs),
            AttributeModify::Remove => attributes.remove(new_attribs),
            AttributeModify::Toggle => attributes.toggle(new_attribs),
        }

        #[cfg(target_arch = "x86_64")]
        {
            if !crate::registers::msr::IA32_EFER::get_nxe() {
                // This bit is reserved if NXE is not supported. For now, this means silently removing it for compatability.
                attributes.remove(PageAttributes::NO_EXECUTE);
            }
        }

        self.0 = (self.0 & !PageAttributes::all().bits()) | attributes.bits();
    }

    /// Clears the page table entry of data, setting all bits to zero.
    pub const fn clear(&mut self) {
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

pub trait TableLevel {}

pub enum Level4 {}
pub enum Level3 {}
pub enum Level2 {}
pub enum Level1 {}

impl TableLevel for Level4 {}
impl TableLevel for Level3 {}
impl TableLevel for Level2 {}
impl TableLevel for Level1 {}

pub trait HeirarchicalLevel: TableLevel {
    type NextLevel: TableLevel;
}
impl HeirarchicalLevel for Level4 {
    type NextLevel = Level3;
}
impl HeirarchicalLevel for Level3 {
    type NextLevel = Level2;
}
impl HeirarchicalLevel for Level2 {
    type NextLevel = Level1;
}

#[repr(C, align(0x1000))]
pub struct PageTable<L: TableLevel>([PageTableEntry; 512], core::marker::PhantomData<L>);

impl<L: TableLevel> PageTable<L> {
    pub const fn new() -> Self {
        Self([PageTableEntry::empty(); 512], core::marker::PhantomData)
    }

    pub unsafe fn clear(&mut self) {
        self.0.fill(PageTableEntry::empty());
    }

    pub fn get_entry(&self, index: usize) -> &PageTableEntry {
        &self.0[index]
    }

    pub fn get_entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.0[index]
    }

    pub fn iter(&self) -> core::slice::Iter<PageTableEntry> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<PageTableEntry> {
        self.0.iter_mut()
    }
}

impl<L: HeirarchicalLevel> PageTable<L> {
    /// Gets an immutable reference to the page table that lies in the given entry index's frame.
    pub unsafe fn sub_table(&self, index: usize, phys_mapped_page: &Page) -> Option<&PageTable<L::NextLevel>> {
        let entry = self.get_entry(index);

        if entry.is_present() {
            Some(
                phys_mapped_page
                    .forward_checked(entry.get_frame_index())
                    .unwrap()
                    .as_ptr::<PageTable<L::NextLevel>>()
                    .as_ref()
                    .unwrap(),
            )
        } else {
            None
        }
    }

    /// Gets an mutable reference to the page table that lies in the given entry index's frame.
    pub unsafe fn sub_table_mut(
        &mut self,
        index: usize,
        phys_mapped_page: &Page,
    ) -> Option<&mut PageTable<L::NextLevel>> {
        let entry = self.get_entry(index);

        if entry.is_present() {
            Some(
                phys_mapped_page
                    .forward_checked(entry.get_frame_index())
                    .unwrap()
                    .as_mut_ptr::<PageTable<L::NextLevel>>()
                    .as_mut()
                    .unwrap(),
            )
        } else {
            None
        }
    }

    /// Attempts to get a mutable reference to  the page table that lies in the given entry index's frame, or
    /// creates the sub page table if it doesn't already exist.
    pub unsafe fn sub_table_create(
        &mut self,
        index: usize,
        phys_mapping_page: &Page,
        frame_manager: &'static crate::memory::FrameManager,
    ) -> &mut PageTable<L::NextLevel> {
        let entry = self.get_entry_mut(index);

        let (frame_index, created) = if entry.is_present() {
            (entry.get_frame_index(), false)
        } else {
            let frame_index = frame_manager.lock_next().unwrap();

            entry.set(frame_index, PageAttributes::PRESENT | PageAttributes::WRITABLE | PageAttributes::USERSPACE);

            (frame_index, true)
        };

        let sub_table_page = phys_mapping_page.forward_checked(frame_index).unwrap();

        // If we created the page, clear it to a known initial state.
        if created {
            sub_table_page.clear_memory();
        }

        sub_table_page.as_mut_ptr::<PageTable<L::NextLevel>>().as_mut().unwrap()
    }
}
