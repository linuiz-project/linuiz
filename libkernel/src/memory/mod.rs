mod block_allocator;
mod frame;
mod frame_allocator;
mod global_memory;
mod page;
mod uefi;

#[cfg(feature = "kernel_impls")]
mod global_allocator;

pub use block_allocator::*;
pub use frame::*;
pub use frame_allocator::*;
pub use global_memory::*;
pub use page::*;
pub use uefi::*;
pub mod mmio;
pub mod paging;

#[cfg(feature = "kernel_impls")]
pub use global_allocator::*;

pub const KIBIBYTE: usize = 0x400; // 1024
pub const MIBIBYTE: usize = KIBIBYTE * KIBIBYTE;

pub const fn to_kibibytes(value: usize) -> usize {
    value / KIBIBYTE
}

pub const fn to_mibibytes(value: usize) -> usize {
    value / MIBIBYTE
}
