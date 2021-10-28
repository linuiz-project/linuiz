use crate::{addr_ty::Virtual, Address};

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
        if addr.is_aligned(0x1000) {
            Self {
                index: addr.page_index(),
            }
        } else {
            panic!("page address is not page-aligned")
        }
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        let ptr_usize = ptr as usize;

        if (ptr_usize % 0x1000) != 0 {
            panic!("page address is not page-aligned")
        } else {
            Self {
                index: ptr_usize / 0x1000,
            }
        }
    }

    pub const fn containing_addr(addr: Address<Virtual>) -> Self {
        Self {
            index: addr.page_index(),
        }
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
        core::ptr::write_bytes::<usize>(
            self.as_mut_ptr(),
            0x0,
            0x1000 / core::mem::size_of::<usize>(),
        );
    }

    pub fn to(&self, count: usize) -> Option<PageIterator> {
        self.forward(count).map(|end| PageIterator::new(self, &end))
    }

    pub const fn forward(&self, count: usize) -> Option<Self> {
        if self.index() <= (usize::MAX - count) {
            Some(Page::from_index(self.index() + count))
        } else {
            None
        }
    }

    pub const fn backward(&self, count: usize) -> Option<Self> {
        if self.index() > (usize::MIN + count) {
            Some(Page::from_index(self.index() - count))
        } else {
            None
        }
    }
}

impl core::iter::Step for Page {
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start.forward(count)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start.backward(count)
    }

    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        Some(end.index() - start.index())
    }
}

impl core::fmt::Debug for Page {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Page")
            .field(&format_args!("0x{:X}", self.index << 12))
            .finish()
    }
}

pub struct PageIterator {
    start: Page,
    current: Page,
    end: Page,
}

impl PageIterator {
    pub fn new(start: &Page, end: &Page) -> Self {
        Self {
            start: *start,
            current: *start,
            end: *end,
        }
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
