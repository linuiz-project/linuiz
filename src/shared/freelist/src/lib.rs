#![no_std]
#![feature(allocator_api)]

extern crate alloc;

use core::alloc::Allocator;

struct Region {
    is_free: bool,
    size: usize
}

pub struct FreeListAllocator<A: Allocator> {
    list: TryVec<Region>,
}
