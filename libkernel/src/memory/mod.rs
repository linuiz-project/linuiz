mod frame_manager;
mod page_manager;

use core::mem::MaybeUninit;

pub use frame_manager::*;
pub use page_manager::*;
pub use paging::*;
pub mod malloc;
pub mod paging;
pub mod uefi;
pub mod volatile;

#[cfg(feature = "global_allocator")]
mod global_alloc {
    use core::{
        cell::{Cell, RefCell},
        lazy::OnceCell,
    };

    use super::MemoryAllocator;
    use crate::cell::SyncCell;

    struct GlobalAllocator<'m>(OnceCell<&'m dyn MemoryAllocator>);

    impl GlobalAllocator {
        pub const fn new() -> Self {
            Self(OnceCell::new())
        }
    }

    unsafe impl GlobalAlloc for GlobalAllocator {
        unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
            (*self)
                .alloc(layout.size(), core::num::NonZeroUsize::new(layout.align()))
                .unwrap()
                .into_parts()
                .0
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
            (*self).dealloc(ptr, layout);
        }
    }

    impl core::ops::Deref for GlobalAllocator {
        type Target = dyn MemoryAllocator;

        fn deref(&self) -> &Self::Target {
            self.0.get().expect("no global allocator")
        }
    }

    #[global_allocator]
    static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

    pub unsafe fn set(malloc: &'static dyn MemoryAllocator) {
        GLOBAL_ALLOCATOR
            .0
            .set(malloc)
            .expect("mallocator already set");
    }

    pub(super) unsafe fn get() -> &'static GlobalAllocator {
        &GLOBAL_ALLOCATOR
    }
}

fn alloc(
    &self,
    size: usize,
    align: Option<core::num::NonZeroUsize>,
) -> Result<SafePtr<u8>, AllocError> {
    unsafe { global_alloc::get() }.alloc(size, align)
}

fn alloc_pages(&self, count: usize) -> Result<(Address<Physical>, SafePtr<u8>), AllocError> {
    unsafe { global_alloc::get() }.alloc_pages(count)
}

fn alloc_against(&self, frame_index: usize, count: usize) -> Result<SafePtr<u8>, AllocError> {
    unsafe { global_alloc::get() }.alloc_against(frame_index, count)
}

fn alloc_identity(&self, frame_index: usize, count: usize) -> Result<SafePtr<u8>, AllocError> {
    unsafe { global_alloc::get() }.alloc_identity(frame_index, count)
}

unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    unsafe { global_alloc::get() }.dealloc(ptr, layout);
}

fn get_page_state(&self, page_index: usize) -> Option<bool> {
    unsafe { global_alloc::get() }.get_page_state(page_index)
}

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

pub trait MemoryAllocator {
    fn alloc(
        &self,
        size: usize,
        align: Option<core::num::NonZeroUsize>,
    ) -> Result<SafePtr<u8>, AllocError>;

    fn alloc_pages(&self, count: usize) -> Result<(Address<Physical>, SafePtr<u8>), AllocError>;

    fn alloc_against(&self, frame_index: usize, count: usize) -> Result<SafePtr<u8>, AllocError>;

    /// Attempts to allocate a 1:1 mapping of virtual memory to its physical memory.
    ///
    /// REMARK:
    ///     This function is required only to offer the same guarantees as `VirtualAddressor::identity_map()`.
    fn alloc_identity(&self, frame_index: usize, count: usize) -> Result<SafePtr<u8>, AllocError>;

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);

    // Returns the page state of the given page index.
    // Option is whether it is mapped
    // `bool` is whether it is allocated to
    fn get_page_state(&self, page_index: usize) -> Option<bool>;
}

impl core::alloc::Allocator for MemoryAllocator {
    fn allocate(&self, layout: core::alloc::Layout) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        self.alloc(layout.size(), )
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        todo!()
    }
}

pub struct MMIO {
    ptr: *mut u8,
    len: usize,
}

impl Drop for MMIO {
    fn drop(&mut self) {
        // Possibly reset frame_range? We don't want to forever lose the pointed-to frames, especially if
        // the frames were locked MMIO in error.

        unsafe {
            alloc::alloc::dealloc(
                self.ptr,
                core::alloc::Layout::from_size_align(self.len, 0x1000).unwrap(),
            )
        };
    }
}

impl MMIO {
    pub unsafe fn new_alloc(
        frame_index: usize,
        count: usize,
        is_mem_writable: bool,
        frame_manager: &'static FrameManager,
        page_manager: &PageManager,
        malloc: &impl malloc::MemoryAllocator,
    ) -> Result<Self, malloc::AllocError> {
        for frame_index in frame_index..(frame_index + count) {
            if let Err(FrameError::TypeConversion { from, to }) =
                frame_manager.try_modify_type(frame_index, FrameType::MMIO)
            {
                panic!(
                    "Attempted to assign MMIO to Frame {}: {:?} into {:?}",
                    frame_index, from, to
                );
            }
        }

        malloc.alloc_against(frame_index, count).map(|data| {
            let parts = data.into_parts();
            let base_index = (parts.0 as usize) / 0x1000;

            for offset in 0..count {
                page_manager.set_page_attribs(
                    &Page::from_index(base_index + offset),
                    PageAttributes::PRESENT
                        | PageAttributes::UNCACHEABLE
                        | PageAttributes::NO_EXECUTE
                        | if is_mem_writable {
                            PageAttributes::WRITABLE | PageAttributes::WRITE_THROUGH
                        } else {
                            PageAttributes::empty()
                        },
                    AttributeModify::Set,
                )
            }

            Self {
                ptr: parts.0,
                len: parts.1,
            }
        })
    }

    pub unsafe fn new(pages: core::ops::Range<Page>) -> Self {
        Self {
            ptr: pages.start.as_mut_ptr(),
            len: pages.count() * 0x1000,
        }
    }

    pub fn mapped_addr(&self) -> crate::Address<crate::Virtual> {
        crate::Address::<crate::Virtual>::from_ptr(self.ptr)
    }

    pub fn pages(&self) -> core::ops::Range<Page> {
        let base_page = paging::Page::from_index((self.ptr as usize) / 0x1000);
        base_page
            ..(base_page
                .forward_checked(crate::align_up_div(self.len, 0x1000))
                .unwrap())
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
            .field("Virtual Base", &self.ptr)
            .field("Length", &self.len)
            .finish()
    }
}
