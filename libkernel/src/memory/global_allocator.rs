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
