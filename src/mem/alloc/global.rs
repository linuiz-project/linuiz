struct DummyAllocator;

// Safety: Allocator does nothing.
unsafe impl core::alloc::GlobalAlloc for DummyAllocator {
    unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
        unimplemented!()
    }

    unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {
        unimplemented!()
    }
}

#[cfg(not(test))]
#[global_allocator]
static DUMMY_ALLOCATOR: DummyAllocator = DummyAllocator;
