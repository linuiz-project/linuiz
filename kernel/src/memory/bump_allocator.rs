use crate::memory::{paging::VirtualAddressorCell, Page};
use core::lazy::OnceCell;
use spin::Mutex;
use x86_64::VirtAddr;

use super::global_lock_next;

pub struct BumpAllocaterCell<'vaddr> {
    allocator: Mutex<OnceCell<BumpAllocator<'vaddr>>>,
}

impl<'vaddr> BumpAllocaterCell<'vaddr> {
    pub const fn empty() -> Self {
        Self {
            allocator: Mutex::new(OnceCell::new()),
        }
    }

    pub fn init(&self, virtual_addressor: &'vaddr VirtualAddressorCell) {
        self.allocator
            .lock()
            .set(BumpAllocator::new(virtual_addressor))
            .ok();
    }
}

struct BumpAllocator<'vaddr> {
    virtual_addressor: &'vaddr VirtualAddressorCell,
    bottom_page: Page,
}

impl<'vaddr> BumpAllocator<'vaddr> {
    fn new(virtual_addressor: &'vaddr VirtualAddressorCell) -> Self {
        Self {
            virtual_addressor,
            // we set bottom page to second page, to avoid using 0x0, which is
            // usually a 'null' address
            bottom_page: Page::from_addr(VirtAddr::new(0x1000)),
        }
    }

    unsafe fn alloc(&mut self, layout: core::alloc::Layout) -> *mut u8 {
        debug!("Kernel allocation: {:?}", layout);
        let start_u64 = self.bottom_page.addr().as_u64();
        let end_u64 = efi_boot::align_down((start_u64 as usize) + layout.size(), 0x1000) as u64;
        for page in Page::range_inclusive(start_u64..end_u64) {
            self.virtual_addressor.map(
                &page,
                &global_lock_next().expect("failed to allocate frames"),
            );
        }

        let old_page = self.bottom_page;
        self.bottom_page = Page::from_addr(VirtAddr::new(end_u64 + 0x1000));
        old_page.addr().as_mut_ptr()
    }

    unsafe fn dealloc(&mut self, _: *mut u8, __: core::alloc::Layout) {}
}

unsafe impl core::alloc::GlobalAlloc for BumpAllocaterCell<'_> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.allocator
            .lock()
            .get_mut()
            .expect("bump allocator has not been configured")
            .alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.allocator
            .lock()
            .get_mut()
            .expect("bump allocator has not been configured")
            .dealloc(ptr, layout);
    }
}
