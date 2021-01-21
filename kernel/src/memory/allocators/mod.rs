mod bump_allocator;
mod frame_allocator;
mod global_memory;

pub use bump_allocator::*;
pub use frame_allocator::*;
pub use global_memory::*;

use core::{alloc::GlobalAlloc, lazy::OnceCell};

pub struct KernelAllocator<'alloc> {
    allocator: OnceCell<BumpAllocator<'alloc>>,
}

impl<'alloc> KernelAllocator<'alloc> {
    pub const fn new() -> Self {
        Self {
            allocator: OnceCell::new(),
        }
    }

    pub fn set_allocator(&self, allocator: BumpAllocator<'alloc>) {
        match self.allocator.set(allocator) {
            Err(_) => panic!("failed to configure kernel allocator"),
            _ => {}
        }
    }
}

unsafe impl GlobalAlloc for KernelAllocator<'_> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.allocator
            .get()
            .expect("kernel allocator has not been configured")
            .alloc(layout)
    }

    unsafe fn dealloc(&self, _: *mut u8, __: core::alloc::Layout) {}
}

#[global_allocator]
static mut GLOBAL_ALLOCATOR: KernelAllocator<'static> = KernelAllocator::new();

pub unsafe fn init_global_allocator(allocator: BumpAllocator<'static>) {
    GLOBAL_ALLOCATOR.set_allocator(allocator);
}
