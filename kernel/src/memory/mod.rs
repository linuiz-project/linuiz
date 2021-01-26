mod bump_allocator;
mod frame;
mod frame_allocator;
mod frame_map;
mod global_memory;
mod page;
mod uefi;

pub mod paging;
pub use bump_allocator::*;
pub use frame::*;
pub use frame_allocator::*;
pub use frame_map::*;
pub use global_memory::*;
pub use page::*;
pub use uefi::*;

pub const KIBIBYTE: usize = 0x400; // 1024
pub const MIBIBYTE: usize = KIBIBYTE * KIBIBYTE;

pub const fn to_kibibytes(value: usize) -> usize {
    value / KIBIBYTE
}

pub const fn to_mibibytes(value: usize) -> usize {
    value / MIBIBYTE
}

#[global_allocator]
static GLOBAL_ALLOCATOR: BumpAllocaterCell<'static> = BumpAllocaterCell::empty();

pub fn init_global_allocator(
    virtual_addressor: &'static crate::memory::paging::VirtualAddressorCell,
) {
    GLOBAL_ALLOCATOR.init(virtual_addressor);
}
