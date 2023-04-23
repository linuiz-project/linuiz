use alloc::{boxed::Box, vec::Vec};
use bitvec::slice::BitSlice;
use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
};
use spin::Mutex;

/// ## Remark
/// This align shouldn't be constant; it needs to be dynamic based upon:
/// * Cache line size
/// * Available memory
/// * Desired memory profile    
const SLAB_LENGTH: usize = 0x1000;

struct Slab<A: Allocator> {
    item_layout: Layout,
    memory: Box<[u8], A>,
    remaining: usize,
    count: usize,
}

fn slabs_can_handle_allocation(size: usize, item_layout: Layout) -> bool {
    let item_count = size / item_layout.pad_to_align().size().next_power_of_two();
    item_count >= usize::try_from(u8::BITS).unwrap()
}

impl<A: Allocator> Slab<A> {
    fn new_in(item_layout: Layout, allocator: A) -> Self {
        let memory = Box::new_zeroed_slice_in(SLAB_LENGTH, allocator);
        let item_count = memory.len() / item_layout.pad_to_align().size().next_power_of_two();

        assert!(item_count >= usize::try_from(u8::BITS).unwrap());

        Self { item_layout, memory: unsafe { memory.assume_init() }, remaining: item_count, count: item_count }
    }

    #[inline]
    const fn count(&self) -> usize {
        self.count
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
    const fn item_layout(&self) -> Layout {
        self.item_layout
    }

    #[inline]
    fn block_size(&self) -> usize {
        self.item_layout().pad_to_align().size().next_power_of_two()
    }

    #[inline]
    fn ledger_byte_range(&self) -> core::ops::Range<usize> {
        0..(self.count / usize::try_from(u8::BITS).unwrap())
    }

    fn ledger(&mut self) -> &mut BitSlice<u8> {
        assert!(self.count >= usize::try_from(u8::BITS).unwrap());
        assert!(self.count.is_power_of_two());

        let ledger_byte_range = self.ledger_byte_range();
        BitSlice::from_slice_mut(&mut self.memory[ledger_byte_range])
    }

    #[inline]
    pub fn check_fits_layout(&self, layout: Layout) -> bool {
        self.item_layout().align() == layout.align() && self.item_layout().size() >= layout.size()
    }

    #[inline]
    pub fn owns_ptr(&self, ptr: *const u8) -> bool {
        self.memory.as_ptr_range().contains(&ptr.cast())
    }

    pub fn next_item(&mut self) -> Option<NonNull<[u8]>> {
        let index = self.ledger().iter_zeros().next()?;
        self.ledger().set(index, true);

        self.remaining -= 1;
        debug_assert!(self.remaining <= self.count);

        let block_size = self.block_size();
        let offset = index * block_size;
        let block_memory = &mut self.memory[offset..(offset + block_size)];
        Some(NonNull::new(block_memory as *mut _).unwrap())
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

        debug_assert!(self.remaining <= self.count);
    }
}

impl<A: Allocator> core::fmt::Debug for Slab<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Slab")
            .field("Block Size", &self.block_size())
            .field("Layout", &self.item_layout())
            .field("Is Empty", &self.is_empty())
            .field("Memory Len", &self.memory.len())
            .finish()
    }
}

pub struct SlabAllocator<A: Allocator> {
    slabs: Mutex<Vec<Slab<A>, A>>,
    max_size: usize,
    allocator: A,
}

// Safety: Type does not use thread-specific logic.
unsafe impl<A: Allocator> Send for SlabAllocator<A> {}
// Safety: Type's mutable conversions are synchronized via `spin::Mutex`.
unsafe impl<A: Allocator> Sync for SlabAllocator<A> {}

impl<A: Allocator + Clone> SlabAllocator<A> {
    #[inline]
    pub fn new_in(max_size_shift: u32, allocator: A) -> Self {
        Self { slabs: Mutex::new(Vec::new_in(allocator.clone())), max_size: 1 << max_size_shift, allocator }
    }
}

unsafe impl<A: Allocator + Clone> Allocator for SlabAllocator<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if !slabs_can_handle_allocation(SLAB_LENGTH, layout) {
            self.allocator.allocate(layout)
        } else {
            let mut slabs = self.slabs.lock();

            let mut item = slabs
                .iter_mut()
                // check not empty
                .filter(|slab| slab.is_empty())
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
