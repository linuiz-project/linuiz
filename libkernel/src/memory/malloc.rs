use crate::{
    addr_ty::{Physical, Virtual},
    cell::SyncRefCell,
    memory::{paging::PageAttributes, Page},
    Address,
};
use core::alloc::Layout;

use super::paging::AttributeModify;

pub trait MemoryAllocator {
    fn minimum_alignment(&self) -> usize;

    // Returns the direct-mapped virtual address for the given physical address.
    unsafe fn physical_memory(&self, addr: Address<Physical>) -> Address<Virtual>;

    // TODO consider returning a slice from this function rather than a raw pointer
    //      reasoning: possibly a more idiomatic way to return a sized chunk of memory
    fn alloc(&self, layout: Layout) -> *mut u8;

    /// Allocates a region of memory pointing to the frame region indicated by
    ///  given the iterator.
    ///
    /// This function assumed the frames are already locked or otherwise valid.
    fn alloc_to(&self, frames: &crate::memory::FrameIterator) -> *mut u8;

    fn dealloc(&self, ptr: *mut u8, layout: Layout);

    fn identity_map(&self, frame: &crate::memory::Frame, virtual_map: bool);

    // Returns the page state of the given page index.
    // Option is whether it is mapped
    // `bool` is whether it is allocated to
    fn page_state(&self, page_index: usize) -> Option<bool>;

    fn get_page_attributes(&self, page: &Page) -> Option<PageAttributes>;
    unsafe fn set_page_attributes(
        &self,
        page: &Page,
        attributes: PageAttributes,
        modify_mode: AttributeModify,
    );

    fn validate_page_tables(&self);
    fn validate_page_branch(&self, page: &Page);
}

static DEFAULT_MALLOCATOR: SyncRefCell<&'static dyn MemoryAllocator> = SyncRefCell::empty();

pub unsafe fn set(allocator: &'static dyn MemoryAllocator) {
    DEFAULT_MALLOCATOR.set(allocator);
}

pub fn get() -> &'static dyn MemoryAllocator {
    *DEFAULT_MALLOCATOR
        .borrow()
        .expect("No default allocator currently assigned.")
}

pub fn try_get() -> Option<&'static dyn MemoryAllocator> {
    DEFAULT_MALLOCATOR.borrow().map(|malloc| *malloc)
}
