use crate::mem::pmm;
use alloc::alloc::Global;
use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    ptr::NonNull,
};

#[global_allocator]
static GLOBAL_ALLOCATOR: KernelAllocator = KernelAllocator;

pub struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        pmm::get()
            .allocate(layout)
            .map_or(core::ptr::null_mut(), |ptr| {
                trace!(
                    "Allocation {:?} -> @{:X?}   0x{:X?}",
                    layout,
                    ptr,
                    ptr.as_ref().len()
                );

                ptr.as_non_null_ptr().as_ptr()
            })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        trace!("Deallocation @{:?}   {:?}", ptr, layout);
        pmm::get().deallocate(NonNull::new(ptr).unwrap(), layout);
    }
}

unsafe impl Allocator for KernelAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, core::alloc::AllocError> {
        pmm::get().allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        pmm::get().deallocate(ptr, layout);
    }
}

pub struct AlignedAllocator<const ALIGN: usize, A: Allocator = Global>(A);

impl<const ALIGN: usize> AlignedAllocator<ALIGN> {
    #[inline]
    pub const fn new() -> Self {
        AlignedAllocator::new_in(Global)
    }
}

impl<const ALIGN: usize, A: Allocator> AlignedAllocator<ALIGN, A> {
    #[inline]
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
