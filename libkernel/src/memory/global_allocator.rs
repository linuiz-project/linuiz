#[global_allocator]
static GLOBAL_ALLOCATOR: crate::memory::BlockAllocator = crate::memory::BlockAllocator::new();

pub unsafe fn init_global_allocator(memory_map: &[crate::memory::uefi::UEFIMemoryDescriptor]) {
    GLOBAL_ALLOCATOR.init(memory_map);
}

pub unsafe fn identity_map(frame: &crate::memory::Frame) {
    GLOBAL_ALLOCATOR.identity_map(frame);
}

pub unsafe fn alloc_to(frames: crate::memory::FrameIterator) -> *mut u8 {
    GLOBAL_ALLOCATOR.alloc_to(frames)
}

pub unsafe fn translate_page(page: &crate::memory::Page) -> Option<crate::memory::Frame> {
    GLOBAL_ALLOCATOR.translate_page(page)
}
