use crate::{Address, Virtual};

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

    pub const fn from_address(addr: Address<Virtual>) -> Self {
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
        formatter.debug_tuple("Page").field(&format_args!("0x{:X}", self.index)).finish()
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
