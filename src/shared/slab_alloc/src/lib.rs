#![cfg_attr(not(test), no_std)]
#![feature(
    allocator_api,          // #32838 <https://github.com/rust-lang/rust/issues/32838>
    new_uninit,             // #63291 <https://github.com/rust-lang/rust/issues/63291>
    pointer_is_aligned,     // #96284 <https://github.com/rust-lang/rust/issues/96284>
    ptr_sub_ptr,            // #95892 <https://github.com/rust-lang/rust/issues/95892>
    drain_filter,           // #43244 <https://github.com/rust-lang/rust/issues/43244>
)]

extern crate alloc;

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};
use bitvec::slice::BitSlice;
use core::{
    alloc::{AllocError, Allocator, Layout},
    num::NonZeroUsize,
    ptr::NonNull,
};
use spin::Mutex;

fn check_valid_slab_size(slab_size: NonZeroUsize) {
    assert!(slab_size.is_power_of_two());
    assert!(slab_size.get() >= (u8::BITS as usize));
}

struct Slab<A: Allocator> {
    block_size: NonZeroUsize,
    memory: Box<[u8], A>,
    remaining: usize,
}

impl<A: Allocator> Slab<A> {
    fn new_in(slab_size: NonZeroUsize, item_layout: Layout, allocator: A) -> Self {
        assert!(slab_size.is_power_of_two());

        let memory = Box::new_zeroed_slice_in(slab_size.get(), allocator);
        let block_size = item_layout.pad_to_align().size().next_power_of_two();
        let block_count = memory.len() / block_size;

        #[cfg(test)]
        println!("SLAB {:?}", memory.as_ptr_range());

        Self {
            block_size: NonZeroUsize::new(block_size).unwrap(),
            memory: unsafe { memory.assume_init() },
            remaining: block_count,
        }
    }

    #[inline]
    const fn len(&self) -> usize {
        self.memory.len() / self.block_size()
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
    fn ledger_bytes(&self) -> usize {
        self.len() / (u8::BITS as usize)
    }

    #[inline]
    fn ledger_bytes_aligned(&self) -> usize {
        (self.ledger_bytes().wrapping_neg() & (1usize << self.block_size().trailing_zeros()).wrapping_neg())
            .wrapping_neg()
    }

    fn non_ledger_memory(&mut self) -> &mut [u8] {
        let ledger_bytes_aligned = self.ledger_bytes_aligned();
        self.memory.get_mut(ledger_bytes_aligned..).unwrap()
    }

    fn ledger(&mut self) -> &mut BitSlice<u8> {
        BitSlice::from_slice_mut(self.memory.get_mut(..self.ledger_bytes()).unwrap())
    }

    #[inline]
    pub fn check_fits_layout(&self, layout: Layout) -> bool {
        self.block_size() >= layout.align() && self.block_size() == layout.size().next_power_of_two()
    }

    #[inline]
    pub fn owns_ptr(&self, ptr: *const u8) -> bool {
        self.memory.as_ptr_range().contains(&ptr.cast())
    }

    pub fn next_item(&mut self) -> Option<NonNull<[u8]>> {
        let index = self.ledger().iter_zeros().next()?;
        self.ledger().set(index, true);
        self.remaining -= 1;

        let block_size = self.block_size();
        let offset = index * block_size;
        let offset_range = offset..(offset + block_size);
        let block_memory = self.non_ledger_memory().get_mut(offset_range).unwrap();

        Some(NonNull::new(block_memory as *mut [u8]).unwrap())
    }

    pub unsafe fn return_item(&mut self, ptr: NonNull<u8>) {
        let ptr = ptr.as_ptr();

        assert!(self.owns_ptr(ptr.cast_const()));

        let offset = ptr.sub_ptr(self.memory.as_ptr());
        let index = offset / self.block_size();

        let ledger = self.ledger();
        assert!(ledger.get(index).is_some(), "len is {} but index is {}", ledger.len(), index);
        assert!(ledger.get(index).unwrap(), "bit is {:?}", *ledger.get(index).unwrap());
        ledger.set(index, false);
        self.remaining += 1;

        debug_assert!(self.remaining() <= self.len());
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

        Self { slab_size, slabs: Mutex::new(Vec::new_in(allocator.clone())), allocator }
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

                    slab.is_full()
                } else {
                    false
                }
            });
        }
    }
}
