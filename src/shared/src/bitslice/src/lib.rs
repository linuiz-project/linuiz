#![no_std]
#![feature(
    slice_ptr_get,                  // #74265 <https://github.com/rust-lang/rust/issues/74265>
    const_slice_index,
)]

use core::{mem::size_of, ptr::NonNull, sync::atomic::AtomicUsize};

pub trait Storage {}

impl Storage for usize {}
impl Storage for AtomicUsize {}

#[repr(transparent)]
pub struct BitRef<S: Storage>(NonNull<S>);

pub struct BitSlice<S: Storage> {
    ptr: NonNull<[S]>,
    len: usize,
}

impl<S: Storage> BitSlice<S> {
    #[inline]
    const fn get_indexes(raw_index: usize) -> (usize, usize) {
        (raw_index / size_of::<S>(), raw_index % size_of::<S>())
    }

    /// # Safety
    ///
    /// `ptr_index` must be within the memory of the internal pointer.
    #[inline]
    unsafe fn get_storage_ptr(&self, ptr_index: usize) -> NonNull<S> {
        debug_assert!(ptr_index < self.ptr.len());
        self.ptr.get_unchecked_mut(ptr_index)
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn get_bit(&self, index: usize) -> Option<&BitRef<S>> {
        if index < self.len() {
            let (ptr_index, _) = Self::get_indexes(index);
            // Safety: Index is checked valid, and reference is known good for `BitRef<S>`.
            Some(unsafe { self.get_storage_ptr(ptr_index).cast::<BitRef<S>>().as_ref() })
        } else {
            None
        }
    }

    #[inline]
    pub fn get_bit_mut(&mut self, index: usize) -> Option<&mut BitRef<S>> {
        if index < self.len() {
            let (ptr_index, _) = Self::get_indexes(index);
            // Safety: Index is checked valid, and reference is known good for `BitRef<S>`.
            Some(unsafe { self.get_storage_ptr(ptr_index).cast::<BitRef<S>>().as_mut() })
        } else {
            None
        }
    }
}

impl<S: Storage> core::ops::Index<usize> for BitSlice<S> {
    type Output = BitRef<S>;

    fn index(&self, index: usize) -> &Self::Output {
        self.get_bit(index).expect("index out of bounds")
    }
}

impl<S: Storage> core::ops::IndexMut<usize> for BitSlice<S> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_bit_mut(index).expect("index out of bounds")
    }
}
