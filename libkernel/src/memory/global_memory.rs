use crate::memory::FrameAllocator;
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

pub unsafe fn init_global_memory(memory_map: &[crate::memory::UEFIMemoryDescriptor]) {
    assert!(
        !GLOBAL_MEMORY.has_allocator(),
        "global memory has already been initialized"
    );

    // calculates total system memory
    let total_memory = memory_map
        .iter()
        .max_by_key(|descriptor| descriptor.phys_start)
        .map(|descriptor| {
            (descriptor.phys_start + (descriptor.page_count * 0x1000)).as_u64() as usize
        })
        .expect("no descriptor with max value");

    info!(
        "Global memory will represent {} MB ({} bytes) of system memory.",
        crate::memory::to_mibibytes(total_memory),
        total_memory
    );

    let frame_alloc_frame_count = FrameAllocator::frame_count_hint(total_memory);
    let frame_alloc_ptr = memory_map
        .iter()
        .find(|descriptor| descriptor.page_count >= (frame_alloc_frame_count as u64))
        .map(|descriptor| descriptor.phys_start.as_u64() as *mut _)
        .expect("failed to find viable memory descriptor for memory map.");

    GLOBAL_MEMORY.set_allocator(FrameAllocator::from_ptr(frame_alloc_ptr, total_memory));
}

pub fn global_memory() -> &'static FrameAllocator<'static> {
    GLOBAL_MEMORY
        .frame_allocator
        .get()
        .expect("global memory has not been configured")
}

pub fn global_top_offset() -> x86_64::VirtAddr {
    x86_64::VirtAddr::new((0x1000000000000 - global_memory().total_memory()) as u64)
}
