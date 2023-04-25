use alloc::{boxed::Box, vec::Vec};
use bitvec::slice::BitSlice;
use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
};
use libsys::page_size;
use spin::Mutex;

/// ## Remark
/// This align shouldn't be constant; it needs to be dynamic based upon:
/// * Cache line size
/// * Available memory
/// * Desired memory profile    
const SLAB_LENGTH: usize = 0x1000;

struct Slab<A: Allocator> {
    block_size: usize,
    memory: Box<[u8], A>,
    remaining: usize,
}

fn slabs_can_handle_allocation(size: usize, item_layout: Layout) -> bool {
    let item_count = size / item_layout.pad_to_align().size().next_power_of_two();
    item_count >= usize::try_from(u8::BITS).unwrap()
}

impl<A: Allocator> Slab<A> {
    fn new_in(item_layout: Layout, allocator: A) -> Self {
        let memory = Box::new_zeroed_slice_in(page_size(), allocator);
        let block_size = item_layout.pad_to_align().size().next_power_of_two();

        let block_count = memory.len() / block_size;
        assert!(u32::try_from(block_count).unwrap() >= u8::BITS);

        Self { block_size, memory: unsafe { memory.assume_init() }, remaining: block_count }
    }

    #[inline]
    const fn len(&self) -> usize {
        self.memory.len() / self.block_size
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
    fn block_size(&self) -> usize {
        self.block_size
    }

    #[inline]
    fn ledger_bytes(&self) -> usize {
        self.len() / (u8::BITS as usize)
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
        let block_size_align_bits = core::num::NonZeroU32::new(block_size.trailing_zeros()).unwrap();
        let ledger_bytes_aligned = libsys::align_up(self.ledger_bytes(), block_size_align_bits);

        let offset = ledger_bytes_aligned + (index * block_size);
        let block_memory = &mut self.memory[offset..(offset + block_size)];
        Some(NonNull::new(block_memory as *mut [u8]).unwrap())
    }

    pub unsafe fn return_item(&mut self, ptr: NonNull<u8>) {
        // TODO check ledger_byte_range to ensure pointer isn't within that

        let ptr = ptr.as_ptr();

        assert!(ptr.is_aligned_to(self.block_size()));
        assert!(self.owns_ptr(ptr.cast_const()));

        let offset = ptr.sub_ptr(self.memory.as_ptr());
        let index = offset / self.block_size();

        self.ledger().set(index, false);
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
    slabs: Mutex<Vec<Slab<A>, A>>,
    allocator: A,
}

// Safety: Type does not use thread-specific logic.
unsafe impl<A: Allocator> Send for SlabAllocator<A> {}
// Safety: Type's mutable conversions are synchronized via `spin::Mutex`.
unsafe impl<A: Allocator> Sync for SlabAllocator<A> {}

impl<A: Allocator + Clone> SlabAllocator<A> {
    #[inline]
    pub fn new_in(allocator: A) -> Self {
        Self { slabs: Mutex::new(Vec::new_in(allocator.clone())), allocator }
    }
}

unsafe impl<A: Allocator + Clone> Allocator for SlabAllocator<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        trace!("Allocation request: {:?}", layout);

        let allocation = {
            if !slabs_can_handle_allocation(SLAB_LENGTH, layout) {
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
                    slabs.push(Slab::new_in(layout, self.allocator.clone()));
                    let slab = slabs.last_mut().unwrap();
                    let old_item = item.replace(slab.next_item().unwrap());

                    assert!(old_item.is_none());
                }

                item.ok_or(AllocError)
            }
        };

        trace!("Allocation served: {:?}", allocation);

        allocation
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if !slabs_can_handle_allocation(SLAB_LENGTH, layout) {
            self.allocator.deallocate(ptr, layout);
        } else {
            let mut slabs = self.slabs.lock();
            let slab = slabs
                .iter_mut()
                .filter(|slab| slab.check_fits_layout(layout))
                .find(|slab| slab.owns_ptr(ptr.as_ptr()))
                .expect("deallocation requested, but no slab owns pointer");

            slab.return_item(ptr);
        }
    }
}
