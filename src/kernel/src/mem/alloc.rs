use crate::mem::{hhdm::Hhdm, pmm::PhysicalMemoryManager};
use alloc::alloc::Global;
use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    num::NonZeroUsize,
    ptr::NonNull,
};
use libsys::{Address, page_shift, page_size};

#[global_allocator]
static GLOBAL_ALLOCATOR: KernelAllocator = KernelAllocator;

pub struct KernelAllocator;

unsafe impl core::alloc::Allocator for KernelAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        assert!(layout.align() <= page_size());

        let frame_count = libsys::align_up_div(layout.size(), page_shift());

        let frame_address = {
            match frame_count {
                0 => unreachable!(
                    "Did not expect `0` from: `libsys::align_up_div({}, {})`",
                    layout.size(),
                    page_shift()
                ),

                1 => PhysicalMemoryManager::next_frame()?,

                frame_count => PhysicalMemoryManager::next_frames(
                    // Safety: `frame_count` is already checked to be `0`.
                    NonZeroUsize::new(frame_count).unwrap(),
                    Some(page_shift()),
                )?,
            }
        };

        trace!("Allocation: {frame_address:?}:{frame_count}");

        Ok(NonNull::slice_from_raw_parts(
            NonNull::without_provenance(
                NonZeroUsize::new(Hhdm::offset().get() + frame_address.get().get()).unwrap(),
            ),
            layout.size(),
        ))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        assert!(layout.align() <= page_size());

        // Calculate the physical (rather than virtual) memory offset of the pointer.
        let phys_offset = ptr.addr().get() - Hhdm::offset().get();
        let phys_offset_aligned = libsys::align_down(phys_offset, page_shift());
        let frame_address = Address::new(phys_offset_aligned).unwrap();

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

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, _: Layout) -> *mut u8 {
        unimplemented!()
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {
        unimplemented!()
    }
}

pub struct AlignedAllocator<const ALIGN: usize, A: Allocator = Global>(A);

impl<const ALIGN: usize> AlignedAllocator<ALIGN> {
    pub const fn new() -> Self {
        AlignedAllocator::new_in(Global)
    }
}

impl<const ALIGN: usize, A: Allocator> AlignedAllocator<ALIGN, A> {
    pub const fn new_in(allocator: A) -> Self {
        Self(allocator)
    }
}

/// # Safety: Type is merely a wrapper for aligned allocation of another allocator impl.
unsafe impl<const ALIGN: usize, A: Allocator> Allocator for AlignedAllocator<ALIGN, A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        match layout.align_to(ALIGN) {
            Ok(layout) => self.0.allocate(layout),
            Err(_) => Err(AllocError),
        }
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        match layout.align_to(ALIGN) {
            Ok(layout) => self.0.allocate_zeroed(layout),
            Err(_) => Err(AllocError),
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        match layout.align_to(ALIGN) {
            // Safety: This function shares the same invariants as `GlobalAllocator::deallocate`.
            Ok(layout) => unsafe { self.0.deallocate(ptr, layout) },
            Err(_) => unimplemented!(),
        }
    }
}
