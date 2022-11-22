#![no_std]
#![feature(allocator_api)]

extern crate alloc;

use core::{alloc::{Allocator, Layout, AllocError}, ptr::NonNull};

use alloc::collections::BTreeMap;

struct Region {
    is_free: bool,
    size: usize
}

pub struct FreeListAllocator<A: Allocator> {
    list: BTreeMap<Region, A>,
}

unsafe impl<A: Allocator> Allocator for FreeListAllocator<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        
    }
} 