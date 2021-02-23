#[global_allocator]
static GLOBAL_ALLOCATOR: crate::memory::BlockAllocator = crate::memory::BlockAllocator::new();

pub unsafe fn init(stack_frames: impl crate::memory::FrameIterator + Clone) {
    GLOBAL_ALLOCATOR.init(stack_frames);
}

pub unsafe fn identity_map(frame: &crate::memory::Frame, map: bool) {
    GLOBAL_ALLOCATOR.identity_map(frame, map);
}

pub unsafe fn alloc_to(frames: impl crate::memory::FrameIterator + Clone) -> *mut u8 {
    GLOBAL_ALLOCATOR.alloc_to(frames)
}

pub unsafe fn translate_page(page: &crate::memory::Page) -> Option<crate::memory::Frame> {
    GLOBAL_ALLOCATOR.translate_page(page)
}

pub fn is_mapped(virt_addr: x86_64::VirtAddr) -> bool {
    GLOBAL_ALLOCATOR.is_mapped(virt_addr)
}

#[macro_export]
macro_rules! alloc {
    ($size:expr) => {
        $crate::alloc!($size, $crate::memory::BlockAllocator::BLOCK_SIZE)
    };
    ($size:expr, $align:expr) => {
        alloc::alloc::alloc(core::alloc::Layout::from_size_align($size, $align).unwrap())
    };
}
