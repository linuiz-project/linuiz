mod block_allocator;
mod frame;
mod frame_allocator;
mod global_memory;
mod page;
mod uefi;

#[cfg(feature = "global_allocator")]
mod global_allocator;

pub use block_allocator::*;
pub use frame::*;
pub use frame_allocator::*;
pub use global_memory::*;
pub use page::*;
pub use uefi::*;
pub mod mmio;
pub mod paging;

pub const KIBIBYTE: usize = 0x400; // 1024
pub const MIBIBYTE: usize = KIBIBYTE * KIBIBYTE;

pub const fn to_kibibytes(value: usize) -> usize {
    value / KIBIBYTE
}

pub const fn to_mibibytes(value: usize) -> usize {
    value / MIBIBYTE
}

use crate::{cell::SyncRefCell, memory::FrameIterator};
use core::alloc::Layout;

pub trait MemoryAllocator {
    fn alloc(&self, layout: Layout) -> *mut u8;
    fn alloc_to(&self, frames: &FrameIterator) -> *mut u8;
    fn dealloc(&self, ptr: *mut u8, layout: Layout);
    fn minimum_alignment(&self) -> usize;
}

static DEFAULT_ALLOCATOR: SyncRefCell<&'static dyn MemoryAllocator> = SyncRefCell::new();

pub fn set_default_allocator(allocator: &'static dyn MemoryAllocator) {
    DEFAULT_ALLOCATOR.set(allocator);
}

pub fn default_allocator() -> &'static dyn MemoryAllocator {
    DEFAULT_ALLOCATOR.get().expect("no default allocator")
}

#[macro_export]
macro_rules! alloc {
    ($size:expr) => {
        $crate::alloc!(
            $size,
            $crate::memory::default_allocator().minimum_alignment()
        ) as *mut _
    };
    ($size:expr, $align:expr) => {
        $crate::memory::default_allocator()
            .alloc(core::alloc::Layout::from_size_align($size, $align).unwrap()) as *mut _
    };
}

#[macro_export]
macro_rules! alloc_to {
    ($frames:expr) => {
        $crate::memory::default_allocator().alloc_to($frames) as *mut _
    };
}
