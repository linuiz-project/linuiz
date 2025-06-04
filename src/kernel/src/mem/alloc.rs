use crate::mem::{
    hhdm,
    pmm::{self, PhysicalMemoryManager},
};
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

// Safety: PMM utilizes interior mutability & Correct:tm: logic.
unsafe impl Allocator for KernelAllocator {
    fn allocate(&self, layout: Layout) -> core::result::Result<NonNull<[u8]>, AllocError> {
        assert!(layout.align() <= page_size());

        let frame_count = libsys::align_up_div(layout.size(), page_shift());
        let frame = match frame_count.cmp(&1usize) {
            core::cmp::Ordering::Greater => PhysicalMemoryManager::next_frames(
                NonZeroUsize::new(frame_count).unwrap(),
                Some(page_shift()),
            ),

            core::cmp::Ordering::Equal => PhysicalMemoryManager::next_frame(),

            core::cmp::Ordering::Less => unreachable!(),
        }
        // TODO log the error somehow
        .map_err(|_| AllocError)?;

        let address = hhdm::get().offset(frame).ok_or(AllocError)?;

        Ok(NonNull::slice_from_raw_parts(
            NonNull::new(address.as_ptr()).unwrap(),
            frame_count * page_size(),
        ))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        assert!(layout.align() <= page_size());

        let offset = ptr.addr().get() - hhdm::get().virt().get();
        let address = Address::new(offset).unwrap();

        if layout.size() <= page_size() {
            PhysicalMemoryManager::free_frame(address).ok();
        } else {
            let frame_count = libsys::align_up_div(layout.size(), page_shift());
            let frames_start = address.index();
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
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout)
            .map_or(core::ptr::null_mut(), |ptr| ptr.as_non_null_ptr().as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Safety: Caller is required to guarantee the provided `ptr` and `layout`
        //         will be valid for a deallocation.
        unsafe {
            self.deallocate(NonNull::new(ptr).unwrap(), layout);
        }
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
