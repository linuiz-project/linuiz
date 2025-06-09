use crate::mem::{hhdm::Hhdm, pmm::PhysicalMemoryManager};
use core::{alloc::Layout, num::NonZeroUsize};
use libsys::{Address, page_shift, page_size};

#[global_allocator]
static GLOBAL_ALLOCATOR: KernelAllocator = KernelAllocator;

pub struct KernelAllocator;

// Safety: Implemented with Correct:tm: logic.
unsafe impl core::alloc::GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert!(layout.align() <= page_size());

        let frame_count = libsys::align_up_div(layout.size(), page_shift());

        let alloc_result = {
            match frame_count {
                0 => unreachable!(
                    "Did not expect `0` from: `libsys::align_up_div({}, {})`",
                    layout.size(),
                    page_shift()
                ),

                1 => PhysicalMemoryManager::next_frame(),

                frame_count => PhysicalMemoryManager::next_frames(
                    // Safety: `frame_count` is already checked to be `0`.
                    unsafe { NonZeroUsize::new_unchecked(frame_count) },
                    None,
                ),
            }
        };

        match alloc_result {
            Ok(frame_address) => {
                trace!("Allocation: {frame_address:?}:{frame_count}");

                core::ptr::without_provenance_mut(Hhdm::frame_to_page(frame_address).get().get())
            }

            Err(crate::mem::pmm::Error::NoneFree) => core::ptr::null_mut(),

            Err(error) => panic!("unresolvable allocation error: {error:?}"),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        assert!(layout.align() <= page_size());

        // Calculate the physical (rather than virtual) memory offset of the pointer.
        let physical_offset = Hhdm::offset_rar(ptr.addr());
        let physical_offset_aligned = libsys::align_down(physical_offset, page_shift());
        let frame_address = Address::new(physical_offset_aligned).unwrap();

        if layout.size() <= page_size() {
            PhysicalMemoryManager::free_frame(frame_address).ok();
        } else {
            let frame_count = libsys::align_up_div(layout.size(), page_shift());
            let frames_start = frame_address.index();
            let frames_end = frames_start + frame_count;

            (frames_start..frames_end)
                .map(Address::from_index)
                .map(Option::unwrap)
                .try_for_each(PhysicalMemoryManager::free_frame)
                .expect("failed while freeing frames");
        }
    }
}
