struct DefaultAllocatorProxy;

impl DefaultAllocatorProxy {
    pub const fn new() -> Self {
        Self {}
    }
}

unsafe impl core::alloc::GlobalAlloc for DefaultAllocatorProxy {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        crate::memory::malloc::get().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        crate::memory::malloc::get().dealloc(ptr, layout);
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: DefaultAllocatorProxy = DefaultAllocatorProxy::new();
