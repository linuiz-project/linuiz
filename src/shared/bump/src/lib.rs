#![no_std]
#![feature(allocator_api)]

use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub struct BumpAllocator<A: Allocator> {
    ptr: spin::Mutex<(usize, NonNull<[u8]>)>,
    allocator: A,
}

impl<A: Allocator> BumpAllocator<A> {
    fn with_ptr_lock<T>(&self, func: impl FnOnce(&mut (usize, NonNull<[u8]>)) -> T) {
        let ptr_lock = self.ptr.lock();
        func(&mut *ptr_lock)
    }
}

unsafe impl<A: Allocator> Allocator for BumpAllocator<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.with_ptr_lock(|(offset, ptr)| {
            use core::mem::align_of;

            let layout_size = layout.pad_to_align().size();
            let aligned_offset = offset.next_multiple_of(layout.align());
            let end_offset = aligned_offset + layout_size;

            if end_offset > ptr.len() {
                *ptr = unsafe {
                    self.allocator.grow(
                        *ptr,
                        Layout::array::<u8>(ptr.len()).map_err(|_| AllocError)?,
                        Layout::array::<u8>(end_offset.next_power_of_two()).map_err(|_| AllocError)?,
                    )?
                };
            }

            if end_offset > ptr.len() {
                Err(AllocError)
            } else {
                *offset = end_offset;

                Ok(unsafe { ptr.get_unchecked_mut(aligned_offset..end_offset) })
            }
        })
    }
}
