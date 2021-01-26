use crate::memory::{Frame, FrameAllocator, FrameIterator};
use core::lazy::OnceCell;

struct GlobalMemory<'global> {
    frame_allocator: OnceCell<FrameAllocator<'global>>,
}

impl<'global> GlobalMemory<'global> {
    const fn new() -> Self {
        Self {
            frame_allocator: OnceCell::new(),
        }
    }

    fn set_allocator(&self, frame_allocator: FrameAllocator<'global>) {
        if let Err(_) = self.frame_allocator.set(frame_allocator) {
            panic!("global memory has already been configured");
        }
    }
}

unsafe impl Sync for GlobalMemory<'_> {}

static GLOBAL_MEMORY: GlobalMemory<'static> = GlobalMemory::new();

pub unsafe fn init_global_memory(memory_map: &[crate::memory::UEFIMemoryDescriptor]) {
    GLOBAL_MEMORY.set_allocator(FrameAllocator::from_mmap(memory_map));
}

fn global_memory() -> &'static FrameAllocator<'static> {
    GLOBAL_MEMORY
        .frame_allocator
        .get()
        .expect("global memory has not been configured")
}

pub unsafe fn global_lock(frame: &Frame) {
    global_memory().lock_frame(frame);
}

pub unsafe fn global_free(frame: &Frame) {
    global_memory().free_frame(frame);
}

pub unsafe fn global_reserve(frame: &Frame) {
    global_memory().reserve_frame(frame);
}

pub unsafe fn global_lock_next() -> Option<Frame> {
    global_memory().lock_next()
}

pub unsafe fn global_lock_count(count: usize) -> Option<FrameIterator> {
    global_memory().lock_next_count(count)
}

pub fn global_total() -> usize {
    global_memory().total_memory()
}

pub fn global_locked() -> usize {
    global_memory().locked_memory()
}

pub fn global_freed() -> usize {
    global_memory().free_memory()
}

pub fn global_reserved() -> usize {
    global_memory().reserved_memory()
}
