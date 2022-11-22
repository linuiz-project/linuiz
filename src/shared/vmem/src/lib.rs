#![no_std]
#![feature(allocator_api)]

extern crate alloc;

use alloc::{collections::{BTreeSet, BTreeMap}, vec::Vec};
use core::{
    alloc::{AllocError, Allocator, Layout},
    ops::Range,
    ptr::NonNull,
};

pub enum Fit {
    Best,
    Instant,
    Next
}

pub struct BoundaryTag {
    start: usize,
    len: usize
}

impl Ord for BoundaryTag {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.start.cmp(&other.start)
    }
}

impl PartialOrd for BoundaryTag {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.start.partial_cmp(&other.start)
    }
}

pub struct Slab<A: Allocator> {
    memory: NonNull<[u8]>,
    table: 
    allocator: A
}


impl<A: Allocator + Clone> Slab<A> {
    fn new_in(, allocator: A) -> Result<Self, AllocError> {
        debug_assert!(layout.size() > 0);

        let padded_layout = layout.pad_to_align();
        let memory = allocator.allocate(unsafe { Layout::from_size_align_unchecked(SLAB_LENGTH, layout.align()) })?;
        let capacity = memory.len() / padded_layout.size();
        let mut list = TryVec::with_capacity_in(capacity, allocator).map_err(|_| AllocError)?;

        for index in 0..capacity {
            let start_offset = index * padded_layout.size();
            let end_offset = start_offset + layout.size();
            list.push(unsafe { memory.get_unchecked_mut(start_offset..end_offset) }).map_err(|_| AllocError)?;
        }

        Ok(Self { layout, capacity, items: list, memory, allocator })
    }
}

impl<A: Allocator> Drop for Slab<A> {
    fn drop(&mut self) {
        unsafe {
            self.allocator.deallocate(
                self.memory.as_non_null_ptr(),
                Layout::from_size_align_unchecked(self.memory.len(), self.layout.align()),
            )
        };
    }
}

pub struct VmemArena<A: Allocator + Clone> {
    min_order: u32,
    segments: BTreeSet<NonNull<[u8]>, A>,
    tags: BTreeMap<NonNull<u8>, usize, A>,
    quantums: Vec<Vec<Slab<A>, A>, A>


    
    allocator: A,
}

impl<A: Allocator + Clone> VmemArena<A> {
    fn 
}

unsafe impl<A: Allocator + Clone> Allocator for VmemArena<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let quantum = QuantumClass::from_layout(layout).ok_or(AllocError)?;

    }
}