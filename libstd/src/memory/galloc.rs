///! Used to repalce Rust's default global allocator with the kernel default mallocator.

struct DefaultAllocatorProxy;

impl DefaultAllocatorProxy {
    pub const fn new() -> Self {
        Self
    }
}

unsafe impl core::alloc::GlobalAlloc for DefaultAllocatorProxy {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        match crate::memory::malloc::get()
            .alloc(layout.size(), core::num::NonZeroUsize::new(layout.align()))
        {
            Ok(alloc) => alloc.into_parts().0 as *mut _,
            Err(_) => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        crate::memory::malloc::get().dealloc(ptr, layout);
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: DefaultAllocatorProxy = DefaultAllocatorProxy::new();
