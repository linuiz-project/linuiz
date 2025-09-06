use core::{num::NonZero, ptr::NonNull};

mod global;

// TODO
// pub static KERNEL_ALLOCATOR: SlabAllocator<FrameAllocator> =
// SlabAllocator::new_in(FrameAllocator);

pub fn allocate_kernel_stack(_pages: NonZero<usize>) -> Option<NonNull<u8>> {
    todo!()
}
