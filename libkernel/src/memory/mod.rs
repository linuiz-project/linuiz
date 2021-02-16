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

pub fn find_stack_descriptor(memory_map: &[UEFIMemoryDescriptor]) -> Option<&UEFIMemoryDescriptor> {
    memory_map.iter().find(|descriptor| {
        descriptor
            .range()
            .contains(&crate::registers::stack::RSP::read().as_u64())
    })
}

#[cfg(feature = "kernel_impls")]
#[global_allocator]
static GLOBAL_ALLOCATOR: BlockAllocator = BlockAllocator::new();

#[cfg(feature = "kernel_impls")]
pub unsafe fn init_global_allocator(memory_map: &[UEFIMemoryDescriptor]) {
    GLOBAL_ALLOCATOR.init(memory_map);
}

#[cfg(feature = "kernel_impls")]
pub unsafe fn identity_map(frame: &Frame) {
    GLOBAL_ALLOCATOR.identity_map(frame);
}

#[cfg(feature = "kernel_impls")]
pub unsafe fn translate_page(page: &Page) -> Option<Frame> {
    GLOBAL_ALLOCATOR.translate_page(page)
}
