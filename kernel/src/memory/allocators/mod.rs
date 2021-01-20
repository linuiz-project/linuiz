mod global_memory;

pub use global_memory::*;

use crate::memory::{paging::VirtualAddessor, Page};
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::RefCell,
};
use efi_boot::{align_down, align_up};
use x86_64::VirtAddr;

struct Allocator<'alloc> {
    allocator: Option<&'alloc mut dyn GlobalAlloc>,
}

impl<'alloc> Allocator<'alloc> {
    const fn uninit() -> Self {
        Self { allocator: None }
    }

    fn replace_internal_allocator(&mut self, allocator: &'alloc mut impl GlobalAlloc) {
        self.allocator = Some(allocator);
    }
}

unsafe impl GlobalAlloc for Allocator<'_> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match &self.allocator {
            Some(allocator) => allocator.alloc(layout),
            None => panic!("allocator has not been initialized"),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        match &self.allocator {
            Some(allocator) => allocator.dealloc(ptr, layout),
            None => panic!("allocator has not been initialized"),
        }
    }
}

pub struct DefaultAllocator<'vaddr> {
    virtual_addessor: RefCell<&'vaddr mut dyn VirtualAddessor>,
    bottom_page: RefCell<Page>,
}

impl<'vaddr> DefaultAllocator<'vaddr> {
    pub fn new(virtual_addessor: &'vaddr mut dyn VirtualAddessor) -> Self {
        Self {
            virtual_addessor: RefCell::new(virtual_addessor),
            bottom_page: RefCell::new(Page::from_addr(VirtAddr::new(0x1000))),
        }
    }
}

unsafe impl GlobalAlloc for DefaultAllocator<'_> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let bottom_addr = self.bottom_page.borrow().addr();

        let bottom_addr_usize = bottom_addr.as_u64() as usize;
        for addr in (bottom_addr_usize..align_down(bottom_addr_usize, 0x1000)).step_by(0x1000) {
            self.virtual_addessor.borrow_mut().map(
                &Page::from_addr(VirtAddr::new(addr as u64)),
                &global_memory_mut(|allocator| {
                    allocator.lock_next().expect("failed to allocate frames")
                }),
            );
        }

        self.bottom_page
            .replace(Page::from_addr(
                bottom_addr + (align_up(layout.size(), 0x1000) as u64),
            ))
            .addr()
            .as_mut_ptr()
    }

    unsafe fn dealloc(&self, _: *mut u8, __: Layout) {}
}

#[global_allocator]
static mut GLOBAL_ALLOCATOR: Allocator<'static> = Allocator::uninit();

pub fn init_global_allocator(allocator: &'static mut impl GlobalAlloc) {
    unsafe { GLOBAL_ALLOCATOR.replace_internal_allocator(allocator) };
}
