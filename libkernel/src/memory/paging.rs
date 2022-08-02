use core::{fmt, marker::PhantomData};
use libarch::{Address, Virtual};

lazy_static::lazy_static! {
    #[cfg(target_arch = "x86_64")]
    pub static ref NXE_SUPPORT: bool = libarch::registers::x86_64::msr::IA32_EFER::get_nxe();
}

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
        unsafe { Address::new_unsafe(self.index * 0x1000) }
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
    pub struct PageAttribute: usize {
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

        const CODE = Self::PRESENT.bits();
        const RODATA = Self::PRESENT.bits() | Self::NO_EXECUTE.bits();
        const DATA = Self::PRESENT.bits() | Self::WRITABLE.bits() | Self::NO_EXECUTE.bits();
        const MMIO = Self::DATA.bits() | Self::UNCACHEABLE.bits();
        const FRAMEBUFFER = Self::DATA.bits();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeModify {
    Set,
    Insert,
    Remove,
    Toggle,
}

// TODO use u64 here
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(usize);

impl PageTableEntry {
    const FRAME_INDEX_MASK: usize = 0x000FFFFF_FFFFF000;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn set(&mut self, frame_index: usize, attributes: PageAttribute) {
        self.0 = (frame_index * 0x1000) | attributes.bits();
    }

    pub const fn get_frame_index(&self) -> Option<usize> {
        if self.get_attribs().contains(PageAttribute::PRESENT) {
            Some((self.0 & Self::FRAME_INDEX_MASK) / 0x1000)
        } else {
            None
        }
    }

    pub const fn set_frame_index(&mut self, frame_index: usize) {
        self.0 = (self.0 & PageAttribute::all().bits()) | (frame_index * 0x1000);
    }

    // Takes this page table entry's frame, even if it is non-present.
    pub const unsafe fn take_frame_index(&mut self) -> usize {
        let frame_index = (self.0 & Self::FRAME_INDEX_MASK) / 0x1000;
        self.0 &= !Self::FRAME_INDEX_MASK;
        frame_index
    }

    pub const fn get_attribs(&self) -> PageAttribute {
        PageAttribute::from_bits_truncate(self.0)
    }

    pub fn set_attributes(&mut self, new_attribs: PageAttribute, modify_mode: AttributeModify) {
        let mut attribs = PageAttribute::from_bits_truncate(self.0);

        match modify_mode {
            AttributeModify::Set => attribs = new_attribs,
            AttributeModify::Insert => attribs.insert(new_attribs),
            AttributeModify::Remove => attribs.remove(new_attribs),
            AttributeModify::Toggle => attribs.toggle(new_attribs),
        }

        #[cfg(target_arch = "x86_64")]
        {
            if !*crate::memory::paging::NXE_SUPPORT {
                // This bit is reserved if NXE is not supported. For now, this means silently removing it for compatability.
                attribs.remove(PageAttribute::NO_EXECUTE);
            }
        }

        self.0 = (self.0 & !PageAttribute::all().bits()) | attribs.bits();
    }

    pub const unsafe fn clear(&mut self) {
        self.0 = 0;
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Page Table Entry")
            .field(&self.get_frame_index())
            .field(&self.get_attribs())
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
pub struct PageTable<L: TableLevel> {
    entries: [PageTableEntry; 512],
    level: PhantomData<L>,
}

impl<L: TableLevel> PageTable<L> {
    pub const fn new() -> Self {
        Self { entries: [PageTableEntry::empty(); 512], level: PhantomData }
    }

    pub unsafe fn clear(&mut self) {
        self.entries.fill(PageTableEntry::empty());
    }

    pub fn get_entry(&self, index: usize) -> &PageTableEntry {
        &self.entries[index]
    }

    pub fn get_entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }

    pub fn iter(&self) -> core::slice::Iter<PageTableEntry> {
        self.entries.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<PageTableEntry> {
        self.entries.iter_mut()
    }
}

impl<L: HeirarchicalLevel> PageTable<L> {
    pub unsafe fn sub_table(&self, index: usize, phys_mapped_page: &Page) -> Option<&PageTable<L::NextLevel>> {
        self.get_entry(index).get_frame_index().map(|frame_index| {
            phys_mapped_page.forward_checked(frame_index).unwrap().as_ptr::<PageTable<L::NextLevel>>().as_ref().unwrap()
        })
    }

    pub unsafe fn sub_table_mut(
        &mut self,
        index: usize,
        phys_mapped_page: &Page,
    ) -> Option<&mut PageTable<L::NextLevel>> {
        self.get_entry_mut(index).get_frame_index().map(|frame_index| {
            phys_mapped_page
                .forward_checked(frame_index)
                .unwrap()
                .as_mut_ptr::<PageTable<L::NextLevel>>()
                .as_mut()
                .unwrap()
        })
    }

    pub unsafe fn sub_table_create(
        &mut self,
        index: usize,
        phys_mapping_page: &Page,
        frame_manager: &'static crate::memory::FrameManager<'_>,
    ) -> &mut PageTable<L::NextLevel> {
        let entry = self.get_entry_mut(index);
        let (frame_index, created) = match entry.get_frame_index() {
            Some(frame_index) => (frame_index, false),
            None => {
                let frame_index = frame_manager.lock_next().unwrap();

                entry.set(frame_index, PageAttribute::PRESENT | PageAttribute::WRITABLE | PageAttribute::USERSPACE);

                (frame_index, true)
            }
        };

        let sub_table_page = phys_mapping_page.forward_checked(frame_index).unwrap();

        // If we created the page, clear it to a known initial state.
        if created {
            sub_table_page.clear_memory();
        }

        sub_table_page.as_mut_ptr::<PageTable<L::NextLevel>>().as_mut().unwrap()
    }
}
