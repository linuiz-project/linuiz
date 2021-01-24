mod bump_allocator;
mod frame_allocator;
mod global_memory;

pub use bump_allocator::*;
pub use frame_allocator::*;
pub use global_memory::*;

#[global_allocator]
static GLOBAL_ALLOCATOR: BumpAllocaterCell<'static> = BumpAllocaterCell::empty();

pub fn init_global_allocator(
    virtual_addressor: &'static crate::memory::paging::VirtualAddressorCell,
) {
    GLOBAL_ALLOCATOR.init(virtual_addressor);
}
