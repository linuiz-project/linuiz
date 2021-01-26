use core::ops::Range;
use x86_64::VirtAddr;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Page(u64);

impl Page {
    pub const fn null() -> Self {
        Self { 0: 0 }
    }

    pub const fn from_index(index: u64) -> Self {
        Self { 0: index }
    }

    pub fn from_addr(virt_addr: VirtAddr) -> Self {
        let addr_u64 = virt_addr.as_u64();
        assert_eq!(
            addr_u64 % 0x1000,
            0,
            "page address is not page-aligned: {:?}",
            virt_addr
        );
        Self {
            0: addr_u64 / 0x1000,
        }
    }

    pub const fn containing_addr(virt_addr: VirtAddr) -> Self {
        Self {
            0: virt_addr.as_u64() / 0x1000,
        }
    }

    pub const fn index(&self) -> u64 {
        self.0
    }

    pub const fn addr(&self) -> VirtAddr {
        VirtAddr::new_truncate(self.0 * 0x1000)
    }

    pub unsafe fn clear(&mut self) {
        core::ptr::write_bytes(self.addr().as_mut_ptr::<u8>(), 0x0, 0x1000);
    }

    pub fn range_inclusive(range: Range<u64>) -> PageIterator {
        PageIterator {
            current: Page::from_addr(VirtAddr::new(range.start)),
            end: Page::from_addr(VirtAddr::new(range.end)),
        }
    }

    pub fn offset(&self, count: u64) -> Self {
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
