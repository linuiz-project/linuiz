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

pub struct Slab<A: Allocator + Clone> {
    items: BTreeSet<NonNull<[u8]>, A>,
    total_items: usize,
    allocator: A
}

impl<A: Allocator + Clone> Drop for Slab<A> {
    fn drop(&mut self) {
        todo!()
    }
}




pub enum VmemSegment {
    SpanMarker(usize),
    BoundaryTag
}

pub struct VmemArena<A: Allocator + Clone> {
    min_order: u32,
    segments: ,
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