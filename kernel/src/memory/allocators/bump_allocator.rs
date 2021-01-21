use crate::memory::{allocators::global_memory_mut, paging::VirtualAddressor, Page};
use core::cell::RefCell;
use spin::Mutex;
use x86_64::VirtAddr;

pub struct BumpAllocator<'vaddr> {
    virtual_addessor: RefCell<&'vaddr VirtualAddressor>,
    bottom_page: RefCell<Page>,
    guard: Mutex<usize>,
}

impl<'vaddr> BumpAllocator<'vaddr> {
    pub fn new(virtual_addessor: &'vaddr VirtualAddressor) -> Self {
        Self {
            virtual_addessor: RefCell::new(virtual_addessor),
            bottom_page: RefCell::new(Page::from_addr(VirtAddr::new(0x1000))),
            guard: Mutex::new(0),
        }
    }
}

unsafe impl core::alloc::GlobalAlloc for BumpAllocator<'_> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.guard.lock();

        let start_u64 = self.bottom_page.borrow().addr().as_u64();
        let end_u64 = efi_boot::align_down((start_u64 as usize) + layout.size(), 0x1000) as u64;
        for page in Page::range(start_u64..end_u64) {
            self.virtual_addessor.borrow_mut().map(
                &page,
                &global_memory_mut(|allocator| {
                    allocator.lock_next().expect("failed to allocate frames")
                }),
            );
        }

        self.bottom_page
            .replace(Page::from_addr(VirtAddr::new(end_u64 + 0x1000)))
            .addr()
            .as_mut_ptr()
    }

    unsafe fn dealloc(&self, _: *mut u8, __: core::alloc::Layout) {}
}
