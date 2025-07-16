use crate::mem::{HigherHalfDirectMap, pmm::PhysicalMemoryManager};
use core::{
    alloc::{AllocError, Allocator, Layout},
    num::NonZero,
    ptr::NonNull,
};
use libsys::{Address, page_shift, page_size};

pub static KERNEL_ALLOCATOR: KernelAllocator = KernelAllocator;

pub struct KernelAllocator;

// Safety: Implemented with Correct™ logic.
unsafe impl Allocator for KernelAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        assert!(layout.align() <= page_size());

        trace!(
            "Allocate: {{ size: {:#X}, align: {:#X} }}",
            layout.size(),
            layout.align()
        );

        let frame_count = libsys::align_up_div(layout.size(), page_shift());

        match frame_count {
            0 => unreachable!(
                "did not expect `0` from: `libsys::align_up_div({}, {})`",
                layout.size(),
                page_shift()
            ),

            1 => PhysicalMemoryManager::next_frame(),

            frame_count => PhysicalMemoryManager::next_frames(
                // Safety: `frame_count` is already checked to be >0.
                unsafe { NonZero::<usize>::new_unchecked(frame_count) },
                None,
            ),
        }
        .map(|frame| {
            trace!("Allocate @ {frame:?}:{frame_count}");

            NonNull::slice_from_raw_parts(
                NonNull::without_provenance(HigherHalfDirectMap::offset(frame.get().get())),
                layout.size(),
            )
        })
        .map_err(|error| {
            error!("Allocate: {error:?}");

            AllocError
        })
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

impl KernelAllocator {
    pub fn allocate_t<T>(&self) -> Result<NonNull<T>, AllocError> {
        let layout = Layout::new::<T>();
        let allocation = self.allocate(layout)?;

        Ok(allocation.as_non_null_ptr().cast())
    }
}
