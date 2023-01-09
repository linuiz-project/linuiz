#![no_std]
#![feature(allocator_api, int_roundings, slice_ptr_get)]

use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
};

struct Bump {
    ptr: NonNull<[u8]>,
    offset: usize,
}

pub struct BumpAllocator<A: Allocator> {
    bump: spin::Mutex<Bump>,
    allocator: A,
}

impl<A: Allocator> BumpAllocator<A> {
    pub fn new_in(initial_size: usize, allocator: A) -> Result<Self, AllocError> {
        Ok(Self {
            bump: spin::Mutex::new(Bump {
                ptr: allocator.allocate(Layout::array::<u8>(initial_size).map_err(|_| AllocError)?)?,
                offset: 0,
            }),
            allocator,
        })
    }

    fn with_ptr_lock<T>(&self, func: impl FnOnce(&mut Bump) -> T) -> T {
        let mut ptr_lock = self.bump.lock();
        func(&mut *ptr_lock)
    }
}

unsafe impl<A: Allocator> Allocator for BumpAllocator<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.with_ptr_lock(|bump| {
            let layout_size = layout.pad_to_align().size();
            let aligned_offset = bump.offset.next_multiple_of(layout.align());
            let end_offset = aligned_offset + layout_size;

            if end_offset > bump.ptr.len() {
                bump.ptr = unsafe {
                    self.allocator.grow(
                        bump.ptr.as_non_null_ptr(),
                        Layout::array::<u8>(bump.ptr.len()).map_err(|_| AllocError)?,
                        Layout::array::<u8>(end_offset.next_power_of_two()).map_err(|_| AllocError)?,
                    )?
                };
            }

            bump.offset = end_offset;
            Ok(unsafe { bump.ptr.get_unchecked_mut(aligned_offset..end_offset) })
        })
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        todo!()
    }
}
