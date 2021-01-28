mod block_allocator;
mod frame;
mod frame_allocator;
mod global_memory;
mod page;
mod uefi;

pub mod paging;
pub use block_allocator::*;
pub use frame::*;
pub use frame_allocator::*;
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
static GLOBAL_ALLOCATOR: BlockAllocator<'static> = BlockAllocator::new(Page::from_addr(unsafe {
    x86_64::VirtAddr::new_unsafe(0x7A12000)
}));

pub unsafe fn set_global_addressor(virtual_addressor: paging::VirtualAddressor) {
    GLOBAL_ALLOCATOR.set_addressor(virtual_addressor);
}
