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
    static __lzg_allocate: LinkedSymbol;
    static __lzg_allocate_zeroed: LinkedSymbol;
    static __lzg_deallocate: LinkedSymbol;
}

type AllocateFn = fn(layout: Layout) -> Result<NonNull<[u8]>, AllocError>;
type DeallocateFn = fn(ptr: NonNull<u8>, layout: Layout);

pub trait LzAllocator: Allocator {
    fn allocate_with<T>(&self, init_fn: impl FnOnce() -> T) -> AllocResult<NonNull<T>> {
        let allocation = self
            .allocate(
                // SAFETY: The layout of structs is required to be valid.
                unsafe { Layout::from_size_align_unchecked(size_of::<T>(), align_of::<T>()) },
            )?
            .cast::<T>();

        unsafe { allocation.as_ptr().write(init_fn()) };

        Ok(allocation)
    }
}

pub struct GlobalAllocator;

impl LzAllocator for GlobalAllocator {}

// SAFETY: Implementation safety of these functions is passed on to the implementations of the
//         statically-linked external functions `__allocate`, `__allocate_zeroed`, and `__deallocate`.
unsafe impl Allocator for GlobalAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: Pointer is required to be valid for `AllocateFn`.
        let try_func = unsafe { __lzg_allocate.as_ptr::<AllocateFn>().as_ref() };
        match try_func {
            Some(func) => func(layout),
            None => unimplemented!(),
        }
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        match unsafe { __lzg_allocate_zeroed.as_ptr::<AllocateFn>().as_ref() } {
            Some(func) => func(layout),
            None => {
                let Some(func) = (unsafe { __lzg_allocate.as_ptr::<AllocateFn>().as_ref() })
                    else { return Err(AllocError) };

                func(layout).map(|allocation| {
                    unsafe { core::ptr::write_bytes(allocation.as_non_null_ptr().as_ptr(), 0, allocation.len()) };
                    allocation
                })
            },
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: Pointer is required to be valid for `AllocateFn`.
        match unsafe { __lzg_deallocate.as_ptr::<DeallocateFn>().as_ref() } {
            Some(func) => func(ptr, layout),
            None => unimplemented!(),
        }
    }
}

pub fn allocate(layout: Layout) -> AllocResult<NonNull<[u8]>> {
    GlobalAllocator.allocate(layout)
}

pub fn allocate_zeroed(layout: Layout) -> AllocResult<NonNull<[u8]>> {
    GlobalAllocator.allocate_zeroed(layout)
}

pub fn allocate_with<T>(init_fn: impl FnOnce() -> T) -> AllocResult<NonNull<T>> {
    GlobalAllocator.allocate_with(init_fn)
}

pub unsafe fn deallocate(ptr: NonNull<u8>, layout: Layout) {
    GlobalAllocator.deallocate(ptr, layout)
}
