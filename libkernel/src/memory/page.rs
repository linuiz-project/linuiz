use x86_64::VirtAddr;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Page(usize);

impl Page {
    #[inline]
    pub const fn null() -> Self {
        Self { 0: 0 }
    }

    #[inline]
    pub const fn from_index(index: usize) -> Self {
        Self { 0: index }
    }

    #[inline]
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

    #[inline]
    pub const fn from_ptr<T>(ptr: *const T) -> Self {
        let ptr_usize = unsafe { ptr as usize };

        if (ptr_usize % 0x1000) != 0 {
            panic!("page address is not page-aligned")
        } else {
            Self {
                0: ptr_usize / 0x1000,
            }
        }
    }

    #[inline]
    pub const fn containing_addr(virt_addr: VirtAddr) -> Self {
        Self {
            0: (virt_addr.as_u64() as usize) / 0x1000,
        }
    }

    #[inline]
    pub const fn index(&self) -> usize {
        self.0
    }

    #[inline]
    pub const fn addr(&self) -> VirtAddr {
        VirtAddr::new_truncate(self.addr_u64())
    }

    #[inline]
    pub const fn addr_u64(&self) -> u64 {
        (self.0 as u64) * 0x1000
    }

    #[inline]
    pub const fn ptr<T>(&self) -> *const T {
        (self.0 * 0x1000) as *const T
    }

    #[inline]
    pub const fn mut_ptr<T>(&self) -> *mut T {
        (self.0 * 0x1000) as *mut T
    }

    #[inline]
    pub const fn iter_count(&self, count: usize) -> PageIterator {
        PageIterator {
            current: Page::from_index(self.0),
            end: self.offset(count),
        }
    }

    pub unsafe fn clear(&mut self) {
        core::ptr::write_bytes(self.addr().as_mut_ptr::<u8>(), 0x0, 0x1000);
    }

    pub fn range_count(start_addr: VirtAddr, count: usize) -> PageIterator {
        PageIterator {
            current: Page::from_addr(start_addr),
            end: Page::from_addr(start_addr + ((count * 0x1000) as u64)),
        }
    }

    #[inline]
    pub const fn offset(&self, count: usize) -> Self {
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
        if self.current.0 < self.end.0 {
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
