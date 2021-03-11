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
pub mod falloc;
pub mod malloc;
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

#[macro_export]
macro_rules! alloc {
    ($size:expr) => {
        $crate::alloc!($size, $crate::memory::malloc::get().minimum_alignment()) as *mut _
    };
    ($size:expr, $align:expr) => {
        $crate::memory::malloc::get()
            .alloc(core::alloc::Layout::from_size_align($size, $align).unwrap()) as *mut _
    };
}

#[macro_export]
macro_rules! alloc_to {
    ($frames:expr) => {
        $crate::memory::malloc::get().alloc_to($frames) as *mut _
    };
}
