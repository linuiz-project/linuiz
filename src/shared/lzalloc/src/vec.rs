use crate::{raw_vec::RawVec, AllocResult, GlobalAllocator};
use core::{alloc::Allocator, num::NonZeroUsize};

pub enum VecError {
    IndexOutOfBounds,
    AllocError,
}

pub type VecResult<T> = core::result::Result<T, VecError>;

#[inline]
fn to_vec_result<T>(alloc_result: AllocResult<T>) -> VecResult<T> {
    alloc_result.map_err(|_| VecError::AllocError)
}

pub struct Vec<T, A: Allocator> {
    buffer: RawVec<T, A>,
    len: usize,
}

impl<T> Vec<T, GlobalAllocator> {
    pub const fn new() -> Self {
        Self { buffer: RawVec::new(), len: 0 }
    }

    pub fn with_capacity(capacity: usize) -> VecResult<Self> {
        to_vec_result(RawVec::with_capacity(capacity).map(|raw_vec| Self { buffer: raw_vec, len: 0 }))
    }
}

impl<T, A: Allocator> Vec<T, A> {
    #[inline]
    pub const fn new_in(allocator: A) -> Self {
        Self { buffer: RawVec::new_in(allocator), len: 0 }
    }

    #[inline]
    pub fn with_capacity_in(capacity: usize, allocator: A) -> VecResult<Self> {
        to_vec_result(RawVec::with_capacity_in(capacity, allocator).map(|raw_vec| Self { buffer: raw_vec, len: 0 }))
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    #[inline]
    pub const fn allocator(&self) -> &A {
        self.buffer.allocator()
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Forces the length of the vector to `new_len`.
    ///
    /// This is a low-level operation that maintains none of the normal
    /// invariants of the type. Normally changing the length of a vector
    /// is done using one of the safe operations instead (`push`, `insert`, etc).
    #[inline]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        debug_assert!(new_len <= self.capacity());

        self.len = new_len;
    }

    #[inline]
    pub const fn as_ptr(&self) -> *const T {
        self.buffer.ptr()
    }

    #[inline]
    pub const unsafe fn as_mut_ptr(&self) -> *mut T {
        self.buffer.ptr_mut()
    }

    pub fn reserve(&mut self, additional: NonZeroUsize) -> VecResult<()> {
        to_vec_result(self.buffer.reserve(self.len(), additional))
    }

    pub fn insert(&mut self, index: usize, element: T) -> VecResult<()> {
        let len = self.len();

        if len == self.buffer.capacity() {
            // ### Safety: Value is non-zero.
            self.reserve(unsafe { NonZeroUsize::new_unchecked(1) })?;
        }

        unsafe {
            {
                let offset_ptr = self.as_mut_ptr().add(index);

                if index < len {
                    offset_ptr.copy_to(offset_ptr.add(1), len - index);
                } else if index == len {
                    // No elements needs to be shifted.
                } else {
                    return Err(VecError::IndexOutOfBounds);
                }

                offset_ptr.write(element);
            }

            self.set_len(len + 1);
        }

        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> VecResult<T> {
        let len = self.len();

        if index >= len {
            return Err(VecError::IndexOutOfBounds);
        }

        unsafe {
            let element;

            {
                let offset_ptr = self.as_mut_ptr().add(index);
                element = offset_ptr.read();
                offset_ptr.copy_from(offset_ptr.add(1), len - index - 1);
            }

            self.set_len(len - 1);

            Ok(element)
        }
    }

    #[inline]
    pub fn push(&mut self, element: T) -> VecResult<()> {
        let len = self.len();

        if len == self.capacity() {
            to_vec_result(self.buffer.reserve_one(len))?;
        }

        unsafe {
            let end_ptr = self.as_mut_ptr().add(len);
            end_ptr.write(element);
            self.set_len(len + 1);
        }

        Ok(())
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        let len = self.len();

        if len > 0 {
            // ### Safety: Length is expected to be decremented.
            unsafe { self.set_len(len - 1) };

            // ### Safety: Pointer address is known-good.
            Some(unsafe { self.as_ptr().add(len).read() })
        } else {
            None
        }
    }

    #[inline]
    pub const fn as_slice<'a>(&'a self) -> &'a [T] {
        // ### Safety: Slice is valid for range so long as vec is valid for range.
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len()) }
    }

    #[inline]
    pub const fn as_slice_mut<'a>(&'a mut self) -> &'a mut [T] {
        // ### Safety: Slice is valid for range so long as vec is valid for range.
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), self.len()) }
    }

    pub fn iter<'a>(&'a self) -> core::slice::Iter<'a, T> {
        self.as_slice().iter()
    }

    pub fn iter_mut<'a>(&'a mut self) -> core::slice::IterMut<'a, T> {
        self.as_slice_mut().iter_mut()
    }
}
