#[global_allocator]
pub static GLOBAL_ALLOCATOR: crate::memory::BlockAllocator = crate::memory::BlockAllocator::new();

#[macro_export]
macro_rules! alloc {
    ($size:expr) => {
        $crate::alloc!($size, $crate::memory::BlockAllocator::BLOCK_SIZE)
    };
    ($size:expr, $align:expr) => {
        alloc::alloc::alloc(core::alloc::Layout::from_size_align($size, $align).unwrap())
    };
}
