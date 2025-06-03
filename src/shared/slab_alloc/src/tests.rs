use alloc::{boxed::Box, vec::Vec};
use core::{mem::MaybeUninit, num::NonZeroUsize};

#[test]
fn vec_realloc_test() {
    let slab_allocator =
        crate::SlabAllocator::new_in(NonZeroUsize::new(0x1000).unwrap(), alloc::alloc::Global);

    let mut total_len = 0;
    for size in 0..10000 {
        let mut b = Vec::new_in(&slab_allocator);
        for val in 0..size {
            b.push(MaybeUninit::new(size + val));
        }

        total_len += b.len();
    }

    println!("{}", total_len)
}
