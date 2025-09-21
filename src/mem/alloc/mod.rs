use crate::mem::{
    HigherHalfDirectMap,
    addr::{
        phys::{FrameAddress, StandardFrame},
        virt::StandardPage,
    },
    pmm::PhysicalMemoryManager,
};
use core::{num::NonZero, ptr::NonNull};

mod global;

pub fn allocate_blocks(count: NonZero<usize>) -> Option<NonNull<[u8]>> {
    PhysicalMemoryManager::next_free_segments(count).map(|frames| {
        let page = HigherHalfDirectMap::frame_to_page::<_, StandardPage>(frames.start);
        let ptr = NonZero::<usize>::try_from(page)
            .map(NonNull::<u8>::with_exposed_provenance)
            .unwrap();

        NonNull::slice_from_raw_parts(ptr, frames.count() * StandardFrame::SIZE_IN_BYTES.get())
    })
}
