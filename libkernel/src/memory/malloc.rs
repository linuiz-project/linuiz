use crate::{
    addr_ty::{Physical, Virtual},
    cell::SyncRefCell,
    memory::paging::PageAttributes,
    Address,
};
use core::alloc::Layout;

pub trait MemoryAllocator {
    fn minimum_alignment(&self) -> usize;
    unsafe fn physical_memory(&self, addr: Address<Physical>) -> Address<Virtual>;

    fn alloc(&self, layout: Layout) -> *mut u8;
    fn alloc_to(&self, frames: &crate::memory::FrameIterator) -> *mut u8;
    fn dealloc(&self, ptr: *mut u8, layout: Layout);
    unsafe fn modify_page_attributes(&self, page: &crate::memory::Page, attributes: PageAttributes);
}

static DEFAULT_MALLOCATOR: SyncRefCell<&'static dyn MemoryAllocator> = SyncRefCell::empty();

pub fn set(allocator: &'static dyn MemoryAllocator) {
    DEFAULT_MALLOCATOR.set(allocator);
}

pub fn get() -> &'static dyn MemoryAllocator {
    *DEFAULT_MALLOCATOR.borrow().expect("no default allocator")
}
