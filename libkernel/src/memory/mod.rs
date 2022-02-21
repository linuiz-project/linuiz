mod frame_manager;
mod page_manager;

pub use frame_manager::*;
pub use page_manager::*;
pub use paging::*;
pub mod malloc;
pub mod paging;
pub mod uefi;
pub mod volatile;

#[cfg(feature = "global_allocator")]
mod global_alloc {
    struct DefaultAllocatorProxy;

    impl DefaultAllocatorProxy {
        pub const fn new() -> Self {
            Self
        }
    }

    unsafe impl core::alloc::GlobalAlloc for DefaultAllocatorProxy {
        unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
            match super::malloc::get()
                .alloc(layout.size(), core::num::NonZeroUsize::new(layout.align()))
            {
                Ok(alloc) => alloc.into_parts().0 as *mut _,
                Err(_) => core::ptr::null_mut(),
            }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
            super::malloc::get().dealloc(ptr, layout);
        }
    }

    #[global_allocator]
    static GLOBAL_ALLOCATOR: DefaultAllocatorProxy = DefaultAllocatorProxy::new();
}

use crate::{
    Address, {Physical, Virtual},
};
use core::{mem::MaybeUninit, ops::Range};

pub const KIBIBYTE: usize = 0x400; // 1024
pub const MIBIBYTE: usize = KIBIBYTE * KIBIBYTE;

pub const fn to_kibibytes(value: usize) -> usize {
    value / KIBIBYTE
}

pub const fn to_mibibytes(value: usize) -> usize {
    value / MIBIBYTE
}

#[macro_export]
macro_rules! alloc {
    ($size:expr) => {
        crate::memory::malloc::get().alloc(size, None)
    };

    ($size:expr, $align:expr) => {
        crate::memory::malloc::get().alloc(size, align)
    };
}

#[macro_export]
macro_rules! alloc_to {
    ($frame_index:expr, $count:expr) => {
        $crate::memory::malloc::get().alloc_against($frame_index, $count)
    };
}

#[macro_export]
macro_rules! alloc_obj {
    () => {
        $crate::memory::alloc_obj()
    };
    ($ty:ty) => {
        $crate::memory::alloc_obj::<$ty>()
    };
}

pub fn alloc_obj<T>() -> *mut T {
    crate::memory::malloc::get()
        .alloc(
            core::mem::size_of::<T>(),
            core::num::NonZeroUsize::new(core::mem::align_of::<T>()),
        )
        .unwrap()
        .cast::<T>()
        .unwrap()
        .into_parts()
        .0
}

pub struct MMIO {
    frame_range: Range<usize>,
    pub ptr: *mut u8,
    pub len: usize,
}

impl Drop for MMIO {
    fn drop(&mut self) {
        // Possibly reset frame_range? We don't want to forever lose the pointed-to frames, especially if
        // the frames were locked MMIO in error.

        unsafe {
            crate::memory::malloc::get().dealloc(
                self.ptr,
                core::alloc::Layout::from_size_align(self.len, 1).unwrap(),
            )
        };
    }
}

impl MMIO {
    pub unsafe fn new(frame_index: usize, count: usize) -> Result<Self, malloc::AllocError> {
        for frame_index in frame_index..(frame_index + count) {
            if let Err(FrameError::TypeConversion { from, to }) =
                FRAME_MANAGER.try_modify_type(frame_index, FrameType::MMIO)
            {
                panic!(
                    "Attempted to assign MMIO to Frame {}: {:?} into {:?}",
                    frame_index, from, to
                );
            }
        }

        malloc::get().alloc_against(frame_index, count).map(|data| {
            let parts = data.into_parts();

            Self {
                frame_range: frame_index..(frame_index + count),
                ptr: parts.0,
                len: parts.1,
            }
        })
    }

    pub fn frames(&self) -> &Range<usize> {
        &self.frame_range
    }

    pub fn phys_addr(&self) -> Address<Physical> {
        Address::<Physical>::new(self.frame_range.start * 0x1000)
    }

    pub fn mapped_addr(&self) -> Address<Virtual> {
        Address::<Virtual>::from_ptr(self.ptr)
    }

    pub fn pages(&self) -> paging::PageIterator {
        let base_page = paging::Page::from_addr(self.mapped_addr());
        paging::PageIterator::new(
            &base_page,
            &base_page.forward(self.frame_range.len()).unwrap(),
        )
    }

    #[inline]
    const fn offset<T>(&self, offset: usize) -> *mut T {
        if (offset + core::mem::size_of::<T>()) < self.len {
            let ptr = unsafe { self.ptr.add(offset).cast::<T>() };

            if ptr.align_offset(core::mem::align_of::<T>()) == 0 {
                return ptr;
            }
        }

        core::ptr::null_mut()
    }

    #[inline]
    pub fn read<T>(&self, offset: usize) -> MaybeUninit<T> {
        unsafe { self.offset::<MaybeUninit<T>>(offset).read_volatile() }
    }

    #[inline]
    pub fn write<T>(&self, offset: usize, value: T) {
        unsafe { self.offset::<T>(offset).write_volatile(value) }
    }

    #[inline(always)]
    pub unsafe fn read_unchecked<T>(&self, offset: usize) -> T {
        core::ptr::read_volatile(self.ptr.add(offset) as *const T)
    }

    #[inline(always)]
    pub unsafe fn write_unchecked<T>(&self, offset: usize, value: T) {
        core::ptr::write_volatile(self.ptr.add(offset) as *mut T, value);
    }

    #[inline]
    pub const unsafe fn borrow<T: volatile::Volatile>(&self, offset: usize) -> &T {
        self.offset::<T>(offset).as_ref().unwrap()
    }

    #[inline]
    pub const unsafe fn slice<'a, T: volatile::Volatile>(
        &'a self,
        offset: usize,
        len: usize,
    ) -> Option<&'a [T]> {
        if (offset + (len * core::mem::size_of::<T>())) < self.len {
            Some(core::slice::from_raw_parts(self.offset::<T>(offset), len))
        } else {
            None
        }
    }
}

impl core::fmt::Debug for MMIO {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MMIO")
            .field("Frames", &self.frame_range)
            .field("Virtual Base", &self.ptr)
            .field("Length", &self.len)
            .finish()
    }
}
