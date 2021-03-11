use crate::{addr_ty::Virtual, memory::FrameAllocator, Address};
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
        if self.frame_allocator.set(frame_allocator).is_err() {
            panic!("global memory has already been configured");
        }
    }

    fn has_allocator(&self) -> bool {
        self.frame_allocator.get().is_some()
    }
}

unsafe impl Sync for GlobalMemory<'_> {}

static GLOBAL_MEMORY: GlobalMemory<'static> = GlobalMemory::new();



pub fn global_memory() -> &'static FrameAllocator<'static> {
    GLOBAL_MEMORY
        .frame_allocator
        .get()
        .expect("global memory has not been configured")
}

pub fn global_top_offset() -> Address<Virtual> {
    Address::<Virtual>::new(0x1000000000000 - global_memory().total_memory(None))
}
