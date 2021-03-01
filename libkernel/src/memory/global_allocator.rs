#[global_allocator]
pub static GLOBAL_ALLOCATOR: crate::memory::BlockAllocator = crate::memory::BlockAllocator::new();

#[macro_export]
macro_rules! alloc {
    ($size:expr) => {
        $crate::alloc!($size, $crate::memory::BlockAllocator::BLOCK_SIZE)
    };
    ($size:expr, $align:expr) => {
        $crate::memory::GLOBAL_ALLOCATOR
            .alloc(core::alloc::Layout::from_size_align($size, $align).unwrap())
    };
}

#[macro_export]
macro_rules! alloc_to {
    ($frames:expr) => {
        $crate::memory::GLOBAL_ALLOCATOR.alloc_to($frames)
    };
}
