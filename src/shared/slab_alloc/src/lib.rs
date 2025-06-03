#![cfg_attr(not(test), no_std)]
#![feature(
    allocator_api,          // #32838 <https://github.com/rust-lang/rust/issues/32838>
    // drain_filter,           // #43244 <https://github.com/rust-lang/rust/issues/43244>
)]

extern crate alloc;

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};
use bitvec::{prelude::BitArray, slice::BitSlice, vec::BitVec};
use core::{
    alloc::{AllocError, Allocator, Layout},
    num::NonZeroUsize,
    ptr::NonNull,
    sync::atomic::AtomicU8,
};
use spin::Mutex;

fn check_valid_slab_size(slab_size: NonZeroUsize) {
    assert!(slab_size.is_power_of_two());
    assert!(slab_size.get() >= (u8::BITS as usize));
}

struct SlabManager<A: Allocator> {
    ledger: BitVec<AtomicU8>,
    slabs: Vec<Box<[u8], A>, A>,
    remaining: usize,
    slab_size: NonZeroUsize,
    block_size: NonZeroUsize,
    allocator: A,
}

impl<A: Allocator + Clone> SlabManager<A> {
    fn new_in(slab_size: NonZeroUsize, block_size: NonZeroUsize, allocator: A) -> Self {
        Self {
            ledger: BitVec::new(),
            slabs: Vec::new_in(allocator.clone()),
            remaining: 0,
            slab_size,
            block_size,
            allocator,
        }
    }

    #[inline]
    const fn len(&self) -> usize {
        self.ledger.len()
    }

    #[inline]
    const fn remaining(&self) -> usize {
        self.remaining
    }

    #[inline]
    const fn is_empty(&self) -> bool {
        self.remaining == 0
    }

    #[inline]
    const fn is_full(&self) -> bool {
        self.remaining == self.len()
    }

    #[inline]
    const fn block_size(&self) -> usize {
        self.block_size.get()
    }

    #[inline]
    pub fn check_fits_layout(&self, layout: Layout) -> bool {
        self.block_size() >= layout.align()
            && self.block_size() == layout.size().next_power_of_two()
    }

    pub fn take_object(&mut self) -> Option<NonNull<[u8]>> {
        let index = self.ledger.first_zero()?;
        self.ledger.set(index, true);
        self.remaining -= 1;

        let bits_per_slab = self.len() / self.slabs.len();
        let slab_index = index / bits_per_slab;
        let slab_block_index = index % bits_per_slab;
        let slab_block_offset = slab_block_index * self.block_size();

        let slab = self.slabs[slab_block_index]
            .get_mut(slab_block_offset..(slab_block_offset + self.block_size()))
            .unwrap();

        Some(NonNull::new(slab as *mut [u8]).unwrap())
    }

    pub unsafe fn return_object(&mut self, ptr: NonNull<u8>) {
        // TODO
    }
}

impl<A: Allocator> core::fmt::Debug for Slab<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Slab")
            .field("Block Size", &self.block_size())
            .field("Is Empty", &self.is_empty())
            .field("Size", &self.memory.len())
            .finish()
    }
}

pub struct SlabAllocator<A: Allocator> {
    slab_size: NonZeroUsize,
    slabs: Mutex<Vec<Slab<A>, A>>,
    allocator: A,
}

// Safety: Type does not use thread-specific logic.
unsafe impl<A: Allocator> Send for SlabAllocator<A> {}
// Safety: Type's mutable conversions are synchronized via `spin::Mutex`.
unsafe impl<A: Allocator> Sync for SlabAllocator<A> {}

impl<A: Allocator + Clone> SlabAllocator<A> {
    #[inline]
    pub fn new_in(slab_size: NonZeroUsize, allocator: A) -> Self {
        check_valid_slab_size(slab_size);

        Self {
            slab_size,
            slabs: Mutex::new(Vec::new_in(allocator.clone())),
            allocator,
        }
    }

    fn max_slabbed_allocation_size(&self) -> usize {
        self.slab_size.get() / (u8::BITS as usize)
    }
}

unsafe impl<A: Allocator + Clone> Allocator for SlabAllocator<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let allocation = {
            if layout.size() > self.max_slabbed_allocation_size() {
                self.allocator.allocate(layout)
            } else {
                let mut slabs = self.slabs.lock();

                let mut item = slabs
                    .iter_mut()
                    // check not empty
                    .filter(|slab| !slab.is_empty())
                    // check size & align fit
                    .filter(|slab| slab.check_fits_layout(layout))
                    // try take & map to allocation result
                    .find_map(|slab| slab.next_item());

                if item.is_none() {
                    slabs.push(Slab::new_in(self.slab_size, layout, self.allocator.clone()));
                    let slab = slabs.last_mut().unwrap();
                    let old_item = item.replace(slab.next_item().unwrap());

                    assert!(old_item.is_none());
                }

                item.ok_or(AllocError)
            }
        };

        allocation
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if layout.size() > self.max_slabbed_allocation_size() {
            self.allocator.deallocate(ptr, layout);
        } else {
            let mut slabs = self.slabs.lock();
            slabs.drain_filter(|slab| {
                if slab.check_fits_layout(layout) && slab.owns_ptr(ptr.as_ptr()) {
                    slab.return_item(ptr);

                    slab.is_empty()
                } else {
                    false
                }
            });
        }
    }
}
