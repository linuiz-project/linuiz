use crate::{Address, Physical};
use alloc::{boxed::Box, vec::Vec};
use core::{
    alloc::Layout,
    mem::{align_of, MaybeUninit},
    panic,
};

#[derive(Debug, Clone, Copy)]
pub enum AllocError {
    TryReserveNonEmptyPage,
    OutOfMemory,
    InvalidAlignment(usize),
    IdentityMappingOverlaps,
    FallocError(crate::memory::FrameError),
}

pub struct SafePtr<T> {
    ptr: *mut T,
    len: usize,
}

impl<T> SafePtr<T> {
    /// Wraps a given `ptr` and `len` in a SafePtr type, allowing safer mutation of the
    /// pointer and its associated memory.
    ///
    /// SAFETY: 
    ///     The following invariants must be met for `SafePtr` to be a valid wrapper over the given memory:
    ///         - `ptr` must be dereferenceable.
    ///         - `ptr + len` must be valid and 'owned' by `ptr`.
    ///         - `ptr` must be valid for the entire lifetime of this struct.
    #[inline]
    pub const unsafe fn new(ptr: *mut T, len: usize) -> Self {
        Self { ptr, len }
    }

    pub const fn cast<U>(self) -> Result<SafePtr<U>, Self> {
        if self.ptr.align_offset(align_of::<U>()) == 0 {
            Ok(SafePtr::<U> {
                ptr: self.ptr as *mut U,
                len: self.len / core::mem::size_of::<U>(),
            })
        } else {
            Err(self)
        }
    }

    pub fn into_uninit_value(self) -> Result<Box<MaybeUninit<T>>, Self> {
        unsafe {
            // TODO this seems odd for some reason, may need to have this work differently.
            if self.len == 1 {
                Ok(Box::from_raw(self.ptr as *mut _))
            } else {
                Err(self)
            }
        }
    }

    pub unsafe fn into_value(self) -> Result<Box<T>, Self> {
        // TODO this seems odd for some reason, may need to have this work differently.
        if self.len == 1 {
            Ok(Box::from_raw(self.ptr))
        } else {
            Err(self)
        }
    }

    pub fn into_uninit_vec(self) -> Vec<MaybeUninit<T>> {
        unsafe { Vec::from_raw_parts(self.ptr as *mut _, self.len, self.len) }
    }

    pub unsafe fn into_vec(self) -> Vec<T> {
        Vec::from_raw_parts(self.ptr, self.len, self.len)
    }

    pub fn into_uninit_slice(self) -> Box<[MaybeUninit<T>]> {
        unsafe {
            Box::from_raw(core::ptr::slice_from_raw_parts_mut(
                self.ptr as *mut _,
                self.len,
            ))
        }
    }

    pub unsafe fn into_slice(self) -> Box<[T]> {
        Box::from_raw(core::ptr::slice_from_raw_parts_mut(self.ptr, self.len))
    }

    #[inline]
    pub const fn into_parts(self) -> (*mut T, usize) {
        (self.ptr, self.len)
    }
}

impl<T: Default + Clone> SafePtr<T> {
    pub fn clear(&mut self) {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }.fill(T::default())
    }
}

impl<T> core::fmt::Debug for SafePtr<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Alloc")
            .field(&self.ptr)
            .field(&self.len)
            .finish()
    }
}



static MEMORY_ALLOCATOR: crate::cell::SyncOnceCell<&'static dyn MemoryAllocator> =
    crate::cell::SyncOnceCell::new();

pub unsafe fn set(allocator: &'static dyn MemoryAllocator) {
    if let Err(_) = MEMORY_ALLOCATOR.set(allocator) {
        panic!("Default memory allocator has already been set.");
    }
}

pub fn get() -> &'static dyn MemoryAllocator {
    *MEMORY_ALLOCATOR.get().unwrap()
}
