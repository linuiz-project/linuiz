#![no_std]
#![feature(allocator_api)]

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
};

#[derive(Debug, Clone, Copy)]
struct Span {
    memory: NonNull<[u8]>,
    refs: usize,
}

impl Eq for Span {}
impl PartialEq for Span {
    fn eq(&self, other: &Self) -> bool {
        self.memory.eq(&other.memory)
    }
}

impl Ord for Span {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.memory.cmp(&other.memory)
    }
}

impl PartialOrd for Span {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.memory.partial_cmp(&other.memory)
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
        self.start.eq(&other.start) && self.len.eq(&other.len)
    }
}

impl Ord for Segment {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        match self.len.cmp(&other.len) {
            core::cmp::Ordering::Equal => self.start.cmp(&other.start),
            ordering => ordering,
        }
    }
}

impl PartialOrd for Segment {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

struct FreeList<A: Allocator + Clone> {
    spans: BTreeSet<Span, A>,
    segments: BTreeSet<Segment, A>,
    allocation_tags: BTreeMap<NonNull<u8>, Span, A>,
}

pub struct FreeListAllocator<A: Allocator + Clone> {
    freelist: spin::Mutex<FreeList<A>>,
    allocator: A,
}

unsafe impl<A: Allocator + Clone> Allocator for FreeListAllocator<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let freelist = self.freelist.lock();

        loop {
            // let segment = freelist.segments.iter().find(|ptr| ptr.len() > )
        }
    }
}
