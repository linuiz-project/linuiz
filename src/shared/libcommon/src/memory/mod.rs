mod aligned_allocator;
mod volatile;

use core::{alloc::AllocError, num::NonZeroUsize};

pub use aligned_allocator::*;
pub use volatile::*;

use crate::{Address, Frame, Virtual};

pub trait InteriorRef {
    type RefType<'a, T>
    where
        T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T;
}

pub struct Ref;
impl InteriorRef for Ref {
    type RefType<'a, T> = &'a T where T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T {
        r
    }
}

pub struct Mut;
impl InteriorRef for Mut {
    type RefType<'a, T> = &'a mut T where T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T {
        &**r
    }
}

// TODO reintroduce MMIO structure

pub trait KernelAllocator: Send + Sync + core::alloc::Allocator {
    fn lock_next(&self) -> Result<Address<Frame>, AllocError>;
    fn lock_next_many(&self, count: NonZeroUsize, alignment: NonZeroUsize) -> Result<Address<Frame>, AllocError>;

    fn lock(&self, frame: Address<Frame>) -> Result<(), AllocError>;
    fn lock_many(&self, frame: Address<Frame>, count: usize) -> Result<(), AllocError>;
    fn free(&self, frame: Address<Frame>) -> Result<(), AllocError>;

    fn allocate_to(&self, frame: Address<Frame>, count: usize) -> Result<Address<Virtual>, AllocError>;

    fn total_memory(&self) -> usize;
}

#[cfg(feature = "global_allocator")]
pub use global_allocator::*;

#[cfg(feature = "global_allocator")]
mod global_allocator {
    use super::KernelAllocator;

    struct GlobalAllocator<'a>(spin::Once<&'a dyn KernelAllocator>);
    // SAFETY: `GlobalAlloc` trait requires `Send`.
    unsafe impl Send for GlobalAllocator<'_> {}
    // SAFETY: `GlobalAlloc` trait requires `Sync`.
    unsafe impl Sync for GlobalAllocator<'_> {}

    /// SAFETY: This struct is a simple wrapper around `GlobalAlloc` itself, and so necessarily implements its safety invariants.
    unsafe impl core::alloc::GlobalAlloc for GlobalAllocator<'_> {
        unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
            match self.0.get().map(|allocator| allocator.allocate(layout)) {
                Some(Ok(ptr)) => ptr.as_mut_ptr(),
                // TODO properly handle abort, via `ud2` handler and perhaps an interrupt flag in fsbase MSR?
                _ => core::intrinsics::abort(),
            }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
            match self.0.get() {
                Some(allocator) => allocator.deallocate(core::ptr::NonNull::new_unchecked(ptr), layout),
                None => core::intrinsics::abort(),
            }
        }
    }

    #[global_allocator]
    static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator(spin::Once::new());

    pub fn set_global_allocator(global_allocator: &'static dyn KernelAllocator) {
        GLOBAL_ALLOCATOR.0.call_once(|| global_allocator);
    }

    pub fn get_global_allocator() -> &'static dyn KernelAllocator {
        *GLOBAL_ALLOCATOR.0.get().unwrap()
    }
}
