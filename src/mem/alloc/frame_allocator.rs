use crate::mem::{HigherHalfDirectMap, pmm::PhysicalMemoryManager};
use core::{
    alloc::{AllocError, Allocator, Layout},
    num::NonZero,
    ptr::NonNull,
};
use libsys::{
    address::{Address, Frame},
    constants::{page_bits, page_size},
    math::{align_down, align_up_div},
};

#[global_allocator]
pub static KERNEL_ALLOCATOR: FrameAllocator = FrameAllocator;

pub struct FrameAllocator;

// Safety: Implemented with Correct™ logic.
unsafe impl Allocator for FrameAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        assert!(layout.align() <= page_size());

        trace!(
            "Allocate: {{ size: {:#X}, align: {:#X} }}",
            layout.size(),
            layout.align()
        );

        let frame_count = align_up_div(layout.size(), page_bits());
        let allocation_frame = {
            match frame_count {
                0 => unreachable!(
                    "did not expect `0` from: `libsys::align_up_div({}, {})`",
                    layout.size(),
                    page_bits()
                ),

                1 => PhysicalMemoryManager::next_free(core::num::NonZero::<usize>::MIN,false),

                frame_count => PhysicalMemoryManager::next_frames(
                    // Safety: `frame_count` is already checked to be >0.
                    unsafe { NonZero::<usize>::new_unchecked(frame_count) },
                    None,
                    false,
                ),
            }
        }
        .map_err(|error| {
            error!("Allocate Error: {error:?}");

            AllocError
        })?;

        let allocation_page = HigherHalfDirectMap::frame_to_page(allocation_frame);
        let allocation_address = NonZero::<usize>::new(allocation_page.get().get())
            .expect("higher-half direct map provided a null address");
        let allocation_ptr = NonNull::<u8>::without_provenance(allocation_address);
        let allocation = NonNull::slice_from_raw_parts(allocation_ptr, layout.size());

        trace!("Allocated: {allocation:X?}");

        Ok(allocation)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        assert!(layout.align() <= page_size());

        trace!(
            "Deallocate: {ptr:#X?} {{ size: {:#X}, align: {:#X} }}",
            layout.size(),
            layout.align()
        );

        // Calculate the physical (rather than virtual) memory offset of the pointer.
        let physical_offset = HigherHalfDirectMap::negative_offset(ptr.addr().get()).get();
        let physical_offset_aligned = align_down(physical_offset, page_bits());
        let frames_start = Address::<Frame>::new(physical_offset_aligned).unwrap();

        if layout.size() <= page_size() {
            if let Err(error) = PhysicalMemoryManager::free_frame(frames_start) {
                error!("Deallocate: {error:?}");
            }
        } else {
            let frame_count = align_up_div(layout.size(), page_bits());
            let frames_end = core::iter::Step::forward(frames_start, frame_count);

            (frames_start..frames_end)
                .try_for_each(PhysicalMemoryManager::free_frame)
                .unwrap_or_else(|error| error!("Deallocate: {error:?}"));
        }
    }
}

// Safety: Perfect code. Perfect. Code.
unsafe impl core::alloc::GlobalAlloc for FrameAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        KERNEL_ALLOCATOR
            .allocate(layout)
            .map(NonNull::as_non_null_ptr)
            .map(NonNull::as_ptr)
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(ptr) = NonNull::new(ptr) else {
            error!("Called `GlobalAlloc::dealloc` with a null pointer.");
            return;
        };

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            KERNEL_ALLOCATOR.deallocate(ptr, layout);
        }
    }
}
