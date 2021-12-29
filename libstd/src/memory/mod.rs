mod frame;
mod page;
mod uefi;

#[cfg(feature = "global_allocator")]
mod galloc;

pub use frame::*;
pub use page::*;
pub use uefi::*;
pub mod falloc;
pub mod malloc;
pub mod paging;
pub mod volatile;

use crate::{
    addr_ty::{Physical, Virtual},
    Address,
};
use core::{mem::MaybeUninit, num::NonZeroUsize, ops::Range};

use self::volatile::Volatile;

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
        $crate::memory::alloc_generic($size, None)
    };
    ($size:expr, $align:expr) => {
        $crate::memory::alloc_generic($size, $align)
    };
}

#[macro_export]
macro_rules! alloc_to {
    ($frame_index:expr, $count:expr) => {
        $crate::memory::malloc::get().alloc_against($frame_index, $count)
    };
}

pub fn alloc_generic<T>(
    size: usize,
    align: Option<NonZeroUsize>,
) -> Result<malloc::Alloc<T>, crate::memory::malloc::AllocError> {
    crate::memory::malloc::get()
        .alloc(
            size * core::mem::size_of::<T>(),
            align.or(core::num::NonZeroUsize::new(core::mem::align_of::<T>())),
        )
        .and_then(|alloc| {
            alloc
                .cast()
                .map_err(|_| crate::memory::malloc::AllocError::InvalidAlignment)
        })
}

pub struct MMIO {
    frame_range: Range<usize>,
    ptr: *mut u8,
    len: usize,
}

impl MMIO {
    // TODO possibly introduct an Address<Frame> type to represent
    // frame addresses?
    pub unsafe fn new(frame_index: usize, count: usize) -> Result<Self, malloc::AllocError> {
        malloc::get()
            .alloc_against(frame_index, count, falloc::FrameState::Reserved)
            .map(|data| {
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

    pub fn pages(&self) -> page::PageIterator {
        let base_page = page::Page::from_addr(self.mapped_addr());
        page::PageIterator::new(
            &base_page,
            &base_page.forward(self.frame_range.len()).unwrap(),
        )
    }

    #[inline]
    fn offset(&self, offset: usize) -> *mut u8 {
        if offset < self.len {
            unsafe { self.ptr.add(offset) }
        } else {
            panic!(
                "Offset is {}, but MMIO ends at offset {}.",
                offset, self.len
            )
        }
    }

    pub fn read<T>(&self, offset: usize) -> MaybeUninit<T> {
        unsafe { (self.offset(offset) as *const MaybeUninit<T>).read_volatile() }
    }

    pub unsafe fn write<T>(&self, offset: usize, value: T) {
        (self.offset(offset) as *mut T).write_volatile(value);
    }

    pub unsafe fn borrow<T: volatile::Volatile>(&self, offset: usize) -> &T {
        (self.offset(offset) as *const T).as_ref().unwrap()
    }

    pub unsafe fn slice<'a, T: Volatile>(&'a self, offset: usize, len: usize) -> &'a [T] {
        if (offset + len) < self.len {
            core::slice::from_raw_parts(self.offset(offset) as *const _, len)
        } else {
            panic!(
                "Offset is {} and len is {}, but MMIO ends at offset {}.",
                offset, len, self.len
            )
        }
    }
}
