use crate::memory::allocators::FrameAllocator;
use core::lazy::OnceCell;
use spin::Mutex;

struct GlobalMemory<'global> {
    frame_allocator: Mutex<OnceCell<FrameAllocator<'global>>>,
}

impl<'global> GlobalMemory<'global> {
    const fn new() -> Self {
        Self {
            frame_allocator: Mutex::new(OnceCell::new()),
        }
    }

    fn set_allocator(&self, frame_allocator: FrameAllocator<'global>) {
        if let Err(_) = self.frame_allocator.lock().set(frame_allocator) {
            panic!("global allocator has already been configured");
        }
    }
}

static GLOBAL_MEMORY: GlobalMemory<'static> = GlobalMemory::new();

pub unsafe fn init_global_memory(memory_map: &[crate::memory::UEFIMemoryDescriptor]) {
    GLOBAL_MEMORY.set_allocator(FrameAllocator::from_mmap(memory_map));
}

pub fn global_memory<C, R>(callback: C) -> R
where
    C: Fn(&FrameAllocator) -> R,
{
    callback(
        GLOBAL_MEMORY
            .frame_allocator
            .lock()
            .get()
            .expect("global allocator has not been configured"),
    )
}

pub fn global_memory_mut<C, R>(mut callback: C) -> R
where
    C: FnMut(&mut FrameAllocator) -> R,
{
    callback(
        GLOBAL_MEMORY
            .frame_allocator
            .lock()
            .get_mut()
            .expect("global allocator has not been configured"),
    )
}

pub fn total_memory_iter() -> core::ops::Range<usize> {
    0..global_memory(|allocator| allocator.total_memory())
}
