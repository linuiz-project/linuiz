use crate::{Address, Virtual};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page(u64);

impl Page {
    #[inline(always)]
    pub const fn null() -> Self {
        Self(0)
    }

    #[inline(always)]
    pub const fn from_index(index: usize) -> Self {
        Self(index as u64)
    }

    /// Constructs a page from the provided address, or `None` if the address is not page-aligned.
    #[inline(always)]
    pub const fn from_address(address: Address<Virtual>) -> Option<Self> {
        if address.is_aligned_to(
            // SAFETY: value is known non-zero
            unsafe { core::num::NonZeroUsize::new_unchecked(0x1000) },
        ) {
            Some(Self(address.page_index() as u64))
        } else {
            None
        }
    }

    #[inline(always)]
    pub const fn from_address_contains(address: Address<Virtual>) -> Self {
        Self(address.page_index() as u64)
    }

    pub fn from_ptr<T>(ptr: *const T) -> Option<Self> {
        if ptr.is_aligned_to(0x1000) {
            Some(Self((ptr as usize as u64) / 0x1000))
        } else {
            None
        }
    }

    #[inline(always)]
    pub const fn range(start_index: usize, end_index: usize) -> core::ops::Range<Self> {
        Self::from_index(start_index)..Self::from_index(end_index)
    }

    #[inline(always)]
    pub const fn index(&self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub const fn address(&self) -> Address<Virtual> {
        crate::Address::<Virtual>::new_truncate(self.0 * 0x1000)
    }

    pub fn forward_checked(&self, count: usize) -> Option<Self> {
        self.index().checked_add(count).map(|new_index| Self::from_index(new_index))
    }

    pub fn backward_checked(&self, count: usize) -> Option<Self> {
        self.index().checked_sub(count).map(|new_index| Self::from_index(new_index))
    }

    /// Clears the 4KiB region from this page's start to its end.
    #[inline(always)]
    pub unsafe fn clear_memory(&self) {
        core::ptr::write_bytes(self.address().as_mut_ptr::<u8>(), 0, 0x1000);
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
        formatter.debug_tuple("Page").field(&format_args!("{:X}", self.index())).finish()
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
        if self.current.0 < self.end.0 {
            let page = self.current.clone();
            self.current.0 += 1;
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
