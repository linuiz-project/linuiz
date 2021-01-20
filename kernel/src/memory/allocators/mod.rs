mod bump_allocator;
mod global_memory;

pub use bump_allocator::*;
pub use global_memory::*;

use spin::Mutex;
use x86_64::VirtAddr;

pub unsafe trait GlobalAllocator {
    unsafe fn alloc(&self, size: usize) -> VirtAddr;
    unsafe fn dealloc(&self, addr: VirtAddr, size: usize);
}

struct Allocator<'alloc> {
    allocator: Option<&'alloc mut dyn GlobalAllocator>,
}

impl<'alloc> Allocator<'alloc> {
    const fn uninit() -> Self {
        Self { allocator: None }
    }

    fn replace_internal_allocator(&mut self, allocator: &'alloc mut impl GlobalAllocator) {
        self.allocator = Some(allocator);
    }
}

unsafe impl GlobalAllocator for Allocator<'_> {
    unsafe fn alloc(&self, size: usize) -> VirtAddr {
        match &self.allocator {
            Some(allocator) => allocator.alloc(size),
            None => panic!("allocator has not been initialized"),
        }
    }

    unsafe fn dealloc(&self, addr: VirtAddr, size: usize) {
        match &self.allocator {
            Some(allocator) => allocator.dealloc(addr, size),
            None => panic!("allocator has not been initialized"),
        }
    }
}

static mut GLOBAL_ALLOCATOR: Mutex<Allocator<'static>> = Mutex::new(Allocator::uninit());

pub unsafe fn init_global_allocator(allocator: &'static mut impl GlobalAllocator) {
    let mut allocator_wrapper = GLOBAL_ALLOCATOR.lock();
    match &allocator_wrapper.allocator {
        Some(_) => panic!("global allocator has already been set"),
        None => {
            warn!("Global allocator is being replaced!");
            allocator_wrapper.replace_internal_allocator(allocator);
        }
    }
}

pub fn alloc<R: Sized>() -> R {
    unsafe {
        core::ptr::read_volatile(
            GLOBAL_ALLOCATOR
                .lock()
                .alloc(core::mem::size_of::<R>())
                .as_mut_ptr(),
        )
    }
}
