// mod frame_manager;
// mod page_manager;
mod page;

// pub use frame_manager::*;
// pub use page_manager::*;
pub use page::*;

pub mod volatile;

/*

   OVERALL L4 INDEX ASSIGNMENTS
   ----------------------------------------
   | 0-255   | Userspace                   |
   ----------------------------------------
   | 256-*** | Physical memory mapping     |
   ----------------------------------------
   | 510     | Kernel core-local state     |
   ----------------------------------------
   | 511     | Kernel ELF memory mappings  |
   ----------------------------------------

*/

pub const PML4_ENTRY_MEM_SIZE: usize = 1 << 9 << 9 << 9 << 12;

// #[derive(Debug)]
// pub enum MMIOError {
//     FramesNotMMIO,
//     FailedFrameTypeModify,
// }

// pub struct MMIO {
//     ptr: *mut u8,
//     len: usize,
// }

// impl MMIO {
//     /// Creates a new MMIO structure wrapping the given region.
//     ///
//     /// `frames` is an [ExactSizeIterator] of frame indexes that this MMIO instance covers.
//     ///
//     /// SAFETY: The caller must ensure that the given frame indexes are valid to be mapped as MMIO.
//     pub unsafe fn new(
//         frames: impl ExactSizeIterator<Item = usize>,
//         frame_manager: &'static FrameManager,
//         page_manager: &PageManager,
//     ) -> Result<Self, MMIOError> {
//         let phys_mem_start_page = page_manager.mapped_page();
//         let initial_page_address = core::cell::OnceCell::new();
//         let mut page_count = 0;

//         for frame_index in frames {
//             // Current frame index in higher-half direct mapped memory.
//             let current_phys_mem_page = phys_mem_start_page.forward_checked(frame_index).unwrap();
//             page_count += 1;

//             // If we haven't set our initial address, set it.
//             if let None = initial_page_address.get() {
//                 initial_page_address.set(current_phys_mem_page).unwrap();
//             }

//             // Attempt to alter the pointed frames type to MMIO.
//             if let Err(FrameError::TypeConversion { from: _, to: _ }) =
//                 frame_manager.try_modify_type(frame_index, FrameType::MMIO)
//             {
//                 return Err(MMIOError::FailedFrameTypeModify);
//             }

//             // Set the correct page attributes for MMIO virtual memory.
//             page_manager.set_page_attributes(
//                 &current_phys_mem_page,
//                 crate::memory::PageAttributes::UNCACHEABLE,
//                 crate::memory::AttributeModify::Insert,
//             );
//         }

//         Ok(Self { ptr: (initial_page_address.get().unwrap().index() * 0x1000) as *mut _, len: page_count * 0x1000 })
//     }

//     pub fn mapped_addr(&self) -> crate::Address<crate::Virtual> {
//         crate::Address::<crate::Virtual>::from_ptr(self.ptr)
//     }

//     #[inline]
//     const fn offset<T>(&self, offset: usize) -> *mut T {
//         if (offset + core::mem::size_of::<T>()) < self.len {
//             let ptr = unsafe { self.ptr.add(offset).cast::<T>() };

//             if ptr.align_offset(core::mem::align_of::<T>()) == 0 {
//                 return ptr;
//             }
//         }

//         core::ptr::null_mut()
//     }

//     #[inline]
//     pub fn read<T>(&self, offset: usize) -> core::mem::MaybeUninit<T> {
//         unsafe { self.offset::<core::mem::MaybeUninit<T>>(offset).read_volatile() }
//     }

//     #[inline]
//     pub fn write<T>(&self, offset: usize, value: T) {
//         unsafe { self.offset::<T>(offset).write_volatile(value) }
//     }

//     #[inline(always)]
//     pub unsafe fn read_unchecked<T>(&self, offset: usize) -> T {
//         core::ptr::read_volatile(self.ptr.add(offset) as *const T)
//     }

//     #[inline(always)]
//     pub unsafe fn write_unchecked<T>(&self, offset: usize, value: T) {
//         core::ptr::write_volatile(self.ptr.add(offset) as *mut T, value);
//     }

//     #[inline]
//     pub const unsafe fn borrow<T: volatile::Volatile>(&self, offset: usize) -> &T {
//         self.offset::<T>(offset).as_ref().unwrap()
//     }

//     #[inline]
//     pub const unsafe fn slice<'a, T: volatile::Volatile>(&'a self, offset: usize, len: usize) -> Option<&'a [T]> {
//         if (offset + (len * core::mem::size_of::<T>())) < self.len {
//             Some(core::slice::from_raw_parts(self.offset::<T>(offset), len))
//         } else {
//             None
//         }
//     }
// }

// impl core::fmt::Debug for MMIO {
//     fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         formatter.debug_struct("MMIO").field("Virtual Base", &self.ptr).field("Length", &self.len).finish()
//     }
// }

use core::alloc::Allocator;

pub struct AlignedAllocator<const ALIGN: usize, A: Allocator>(pub A);

unsafe impl<const ALIGN: usize, A: Allocator> Allocator for AlignedAllocator<ALIGN, A> {
    fn allocate(&self, layout: core::alloc::Layout) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        match layout.align_to(ALIGN) {
            Ok(layout) => self.0.allocate(layout),
            Err(_) => Err(core::alloc::AllocError),
        }
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        match layout.align_to(ALIGN) {
            Ok(layout) => self.0.deallocate(ptr, layout),
            Err(_) => alloc::alloc::handle_alloc_error(layout),
        }
    }
}

/// Provides a type alias around the default global allocator, always providing page-aligned allocations.
pub fn page_aligned_allocator() -> AlignedAllocator<0x1000, alloc::alloc::Global> {
    AlignedAllocator::<0x1000, _>(alloc::alloc::Global)
}

pub fn stack_aligned_allocator() -> AlignedAllocator<0x10, alloc::alloc::Global> {
    AlignedAllocator::<0x10, _>(alloc::alloc::Global)
}

/// Simple type alias for a page-aligned `Box<T>`.
pub type PageAlignedBox<T> = alloc::boxed::Box<T, AlignedAllocator<0x1000, alloc::alloc::Global>>;

pub type StackAlignedBox<T> = alloc::boxed::Box<T, AlignedAllocator<0x10, alloc::alloc::Global>>;
