use core::ops::Range;
use x86_64::VirtAddr;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Page(usize);

impl Page {
    pub const fn null() -> Self {
        Self { 0: 0 }
    }

    pub const fn from_index(index: usize) -> Self {
        Self { 0: index }
    }

    pub const fn from_addr(virt_addr: VirtAddr) -> Self {
        let addr_usize = virt_addr.as_u64() as usize;

        if (addr_usize % 0x1000) != 0 {
            panic!("page address is not page-aligned")
        } else {
            Self {
                0: addr_usize / 0x1000,
            }
        }
    }

    pub const fn containing_addr(virt_addr: VirtAddr) -> Self {
        Self {
            0: (virt_addr.as_u64() as usize) / 0x1000,
        }
    }

    pub const fn index(&self) -> usize {
        self.0
    }

    pub const fn addr(&self) -> VirtAddr {
        VirtAddr::new_truncate((self.0 as u64) * 0x1000)
    }

    pub unsafe fn clear(&mut self) {
        core::ptr::write_bytes(self.addr().as_mut_ptr::<u8>(), 0x0, 0x1000);
    }

    pub fn range_inclusive(range: Range<usize>) -> PageIterator {
        PageIterator {
            current: Page::from_addr(VirtAddr::new(range.start as u64)),
            end: Page::from_addr(VirtAddr::new(range.end as u64)),
        }
    }

    pub fn offset(&self, count: usize) -> Self {
        Self { 0: self.0 + count }
    }
}

pub struct PageIterator {
    current: Page,
    end: Page,
}

impl Iterator for PageIterator {
    type Item = Page;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.0 <= self.end.0 {
            let page = self.current.clone();
            self.current.0 += 1;
            Some(page)
        } else {
            None
        }
    }
}

impl core::fmt::Debug for Page {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Page").field(&self.addr()).finish()
    }
}
