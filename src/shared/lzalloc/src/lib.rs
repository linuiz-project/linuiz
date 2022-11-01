#![cfg_attr(not(test), no_std)]
#![feature(
    core_intrinsics,
    allocator_api,      // #32838 <https://github.com/rust-lang/rust/issues/32838>
    extern_types,       // #43467 <https://github.com/rust-lang/rust/issues/43467>
    slice_ptr_get,      // #74265 <https://github.com/rust-lang/rust/issues/74265>
    const_mut_refs,     // #57349 <https://github.com/rust-lang/rust/issues/57349>
    const_slice_from_raw_parts_mut,
    const_option_ext,
    const_cmp
)]

pub mod deque;
pub mod raw_vec;
pub mod vec;

use core::{
    alloc::{AllocError, Allocator, Layout},
    mem::{align_of, size_of},
    num::NonZeroUsize,
    ptr::NonNull,
};

#[inline]
pub const fn next_capacity(capacity: usize) -> usize {
    core::cmp::max(if capacity.is_power_of_two() { capacity } else { capacity.next_power_of_two() }, 4)
}

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
                // ### Safety: The layout of structs is required to be valid.
                unsafe { Layout::from_size_align_unchecked(size_of::<T>(), align_of::<T>()) },
            )?
            .as_non_null_ptr()
            .cast::<T>();

        unsafe { allocation.as_ptr().write(init_fn()) };

        Ok(allocation)
    }

    fn allocate_slice<T: Clone>(&self, len: NonZeroUsize, value: T) -> AllocResult<&mut [T]> {
        let Ok(layout) = Layout::array::<T>(len.get()) else { return Err(AllocError) };
        let t_ptr = self.allocate(layout)?.as_non_null_ptr().cast::<T>();
        // ### Safety: Size and alignment will be valid due to `.allocate()`s guarantees, and values will be initialized
        //         before the calling context can access the slice memory.
        let t_slice = unsafe { core::slice::from_raw_parts_mut(t_ptr.as_ptr(), len.get()) };
        t_slice.fill(value);

        Ok(t_slice)
    }

    fn allocate_slice_with<T>(&self, len: NonZeroUsize, init_fn: impl FnMut() -> T) -> AllocResult<&mut [T]> {
        let Ok(layout) = Layout::array::<T>(len.get()) else { return Err(AllocError) };
        let t_ptr = self.allocate(layout)?.as_non_null_ptr().cast::<T>();
        // ### Safety: Size and alignment will be valid due to `.allocate()`s guarantees, and values will be initialized
        //         before the calling context can access the slice memory.
        let t_slice = unsafe { core::slice::from_raw_parts_mut(t_ptr.as_ptr(), len.get()) };
        t_slice.fill_with(init_fn);

        Ok(t_slice)
    }
}

pub struct GlobalAllocator;

impl LzAllocator for GlobalAllocator {}

// ### Safety: Implementation safety of these functions is passed on to the implementations of the
//         statically-linked external functions `__allocate`, `__allocate_zeroed`, and `__deallocate`.
unsafe impl Allocator for GlobalAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // ### Safety: Pointer is required to be valid for `AllocateFn`.
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
            }
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // ### Safety: Pointer is required to be valid for `AllocateFn`.
        match unsafe { __lzg_deallocate.as_ptr::<DeallocateFn>().as_ref() } {
            Some(func) => func(ptr, layout),
            None => unimplemented!(),
        }
    }
}

pub fn allocate(layout: Layout) -> AllocResult<NonNull<[u8]>> {
    GlobalAllocator.allocate(layout)
}

pub unsafe fn deallocate(ptr: NonNull<u8>, layout: Layout) {
    GlobalAllocator.deallocate(ptr, layout)
}

pub fn allocate_zeroed(layout: Layout) -> AllocResult<NonNull<[u8]>> {
    GlobalAllocator.allocate_zeroed(layout)
}

pub fn allocate_with<T>(init_fn: impl FnOnce() -> T) -> AllocResult<NonNull<T>> {
    GlobalAllocator.allocate_with(init_fn)
}

pub fn allocate_slice<'a, T: Clone>(len: NonZeroUsize, value: T) -> AllocResult<&'a mut [T]> {
    GlobalAllocator.allocate_slice(len, value)
}

pub fn allocate_slice_with<'a, T>(len: NonZeroUsize, init_fn: impl FnMut() -> T) -> AllocResult<&'a mut [T]> {
    GlobalAllocator.allocate_slice_with(len, init_fn)
}

pub struct AlignedAllocator<const ALIGN: usize>;

unsafe impl<const ALIGN: usize> Allocator for AlignedAllocator<ALIGN> {
    fn allocate(&self, layout: core::alloc::Layout) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        match layout.align_to(ALIGN) {
            Ok(layout) => GlobalAllocator.allocate(layout),
            Err(_) => Err(core::alloc::AllocError),
        }
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        match layout.align_to(ALIGN) {
            Ok(layout) => GlobalAllocator.deallocate(ptr, layout),
            Err(_) => unimplemented!(),
        }
    }
}
