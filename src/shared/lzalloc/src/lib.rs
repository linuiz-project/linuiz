#![no_std]
#![feature(
    ptr_internals,
    core_intrinsics,
    allocator_api,  // #32838 <https://github.com/rust-lang/rust/issues/32838>
    extern_types,   // #43467 <https://github.com/rust-lang/rust/issues/43467>
    // REMARK: Not sure if this feature should be used. Doesn't seem particularly close to stable?
    slice_ptr_get   // #74265 <https://github.com/rust-lang/rust/issues/74265>
)]

pub mod raw_vec;
pub mod vec;

use core::{
    alloc::{AllocError, Allocator, Layout},
    mem::{align_of, size_of},
    ptr::NonNull,
};

pub type AllocResult<T> = core::result::Result<T, AllocError>;

extern "C" {
    type LinkedSymbol;
}

impl LinkedSymbol {
    #[inline]
    pub fn as_ptr<T>(&'static self) -> *const T {
        self as *const _ as *const T
    }
}

extern "C" {
    static __allocate: LinkedSymbol;
    static __allocate_zeroed: LinkedSymbol;
    static __deallocate: LinkedSymbol;
}

type AllocateFn = fn(layout: Layout) -> Result<NonNull<[u8]>, AllocError>;
type DeallocateFn = fn(ptr: NonNull<u8>, layout: Layout);

pub struct GlobalAllocator;

// SAFETY: Implementation safety of these functions is passed on to the implementations of the
//         statically-linked external functions `__allocate`, `__allocate_zeroed`, and `__deallocate`.
unsafe impl Allocator for GlobalAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        Self::allocate(layout)
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        Self::allocate_zeroed(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        Self::deallocate(ptr, layout);
    }
}

impl GlobalAllocator {
    fn allocate(layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: Pointer is required to be valid for `AllocateFn`.
        let try_func = unsafe { __allocate.as_ptr::<AllocateFn>().as_ref() };
        match try_func {
            Some(func) => func(layout),
            None => unimplemented!(),
        }
    }

    fn allocate_zeroed(layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: Pointer is required to be valid for `AllocateFn`.
        let try_func = unsafe {
            __allocate_zeroed.as_ptr::<AllocateFn>().as_ref().or_else(|| __allocate.as_ptr::<AllocateFn>().as_ref())
        };
        match try_func {
            Some(func) => func(layout),
            None => unimplemented!(),
        }
    }

    unsafe fn deallocate(ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: Pointer is required to be valid for `AllocateFn`.
        match unsafe { __deallocate.as_ptr::<DeallocateFn>().as_ref() } {
            Some(func) => func(ptr, layout),
            None => unimplemented!(),
        }
    }

    pub fn allocate_with<T>(init_fn: impl FnOnce() -> T) -> AllocResult<&'static mut T> {
        let allocation = Self::allocate(
            // SAFETY: The layout of structs is required to be valid.
            unsafe { Layout::from_size_align_unchecked(size_of::<T>(), align_of::<T>()) },
        )?;
        let Some(t_mut) = (unsafe { allocation.as_non_null_ptr().as_ptr().cast::<T>().as_mut() })
            else { return Err(AllocError) };
        *t_mut = init_fn();

        Ok(t_mut)
    }

    #[cfg(feature = "bytemuck")]
    pub fn allocate_static<T: bytemuck::NoUninit + bytemuck::AnyBitPattern>(
    ) -> Result<&'static mut T, core::alloc::AllocError> {
        Self::allocate(
            // SAFETY: The layout of structs is required to be valid.
            unsafe { core::alloc::Layout::from_size_align_unchecked(size_of::<T>(), align_of::<T>()) },
        )
        .and_then(|mut ptr| {
            bytemuck::try_from_bytes_mut(
                // SAFETY: Safety requirements of `.as_mut` are upheld by the allocator & bytemuck APIs.
                unsafe { ptr.as_mut() },
            )
            .map_err(|_| AllocError)
        })
    }

    #[cfg(feature = "bytemuck")]
    pub unsafe fn allocate_static_zeroed<T: bytemuck::Zeroable>() -> Result<&'static mut T, core::alloc::AllocError> {
        Self::allocate_zeroed(
            // SAFETY: The layout of structs is required to be valid.
            unsafe { core::alloc::Layout::from_size_align_unchecked(size_of::<T>(), align_of::<T>()) },
        )
        .map(|ptr| {
            // SAFETY: Safety requirements are upheld by `Allocator` & `bytemuck` APIs.
            unsafe { &mut *ptr.as_mut_ptr().cast::<T>() }
        })
    }
}
