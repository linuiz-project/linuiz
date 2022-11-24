#![no_std]
#![feature(allocator_api)]

extern crate alloc;

use alloc::{collections::BTreeSet, vec::Vec};
use core::{
    alloc::{AllocError, Allocator, Layout},
    ops::Range,
    ptr::NonNull,
};

pub enum Fit {
    Best,
    Instant,
    Next,
}

pub struct Slab<A: Allocator + Clone> {
    items: BTreeSet<NonNull<[u8]>, A>,
    total_items: usize,
    allocator: A,
}

impl<A: Allocator + Clone> Drop for Slab<A> {
    fn drop(&mut self) {
        todo!()
    }
}

#[derive(Debug, Clone, Copy)]
struct Span {
    memory: NonNull<[u8]>,
    refs: usize,
}

impl Eq for Span {}
impl PartialEq for Span {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start
    }
}

impl Ord for Span {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.start.cmp(&other.start)
    }
}

impl PartialOrd for Span {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.start.partial_cmp(&other.start)
    }
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    start: usize,
    len: usize,
    span: Span,
}

impl Eq for Segment {}
impl PartialEq for Segment {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start
    }
}

impl Ord for Segment {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.start.cmp(&other.start)
    }
}

impl PartialOrd for Segment {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.start.partial_cmp(&other.start)
    }
}

pub struct Vmem<A: Allocator + Clone> {
    spans: BTreeSet<Span, A>,
    segments: BTreeSet<Segment, A>,
    quantum_order: u32,
    quantums: Vec<Slab<A>, A>,
}

impl<A: Allocator + Clone> Vmem<A> {
    fn new_in(quantum_order: u32, orders: u32, allocator: A) -> Result<Self, AllocError> {
        let quantums = Vec::new_in(allocator.clone());
        for _ in 0..orders {
            quantums.push(Slab::)
        }
    }
}

unsafe impl<A: Allocator + Clone> Allocator for Vmem<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        todo!()
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        todo!()
    }
}
