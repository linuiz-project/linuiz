mod block_allocator;
mod frame;
mod frame_allocator;
mod global_memory;
mod page;
mod uefi;

pub mod paging;

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

struct NoAllocator;

unsafe impl core::alloc::GlobalAlloc for NoAllocator {
    unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
        0x0 as *mut u8
    }

    unsafe fn dealloc(&self, _: *mut u8, __: core::alloc::Layout) {}
}

#[global_allocator]
static GLOBAL_ALLOCATOR: NoAllocator = NoAllocator;

// pub fn init_global_allocator(
//     virtual_addressor: &'static crate::memory::paging::VirtualAddressorCell,
// ) {
//     GLOBAL_ALLOCATOR.init(virtual_addressor);
// }
