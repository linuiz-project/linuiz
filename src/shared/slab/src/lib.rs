#![no_std]
#![feature(
    allocator_api,                  // #32838 <https://github.com/rust-lang/rust/issues/32838>
    strict_provenance,              // #95228 <https://github.com/rust-lang/rust/issues/95228>
    nonnull_slice_from_raw_parts,   // #71941 <https://github.com/rust-lang/rust/issues/71941>
    pointer_is_aligned,             // #96284 <https://github.com/rust-lang/rust/issues/96284>
    ptr_as_uninit,                  // #75402 <https://github.com/rust-lang/rust/issues/75402>
    slice_ptr_get,                  // #74265 <https://github.com/rust-lang/rust/issues/74265>
    maybe_uninit_slice,             // #63569 <https://github.com/rust-lang/rust/issues/63569>
    int_roundings,                  // #88581 <https://github.com/rust-lang/rust/issues/88581>
    nonzero_min_max,                // #89065 <https://github.com/rust-lang/rust/issues/89065>
    const_mut_refs,
    const_option,
    const_option_ext,
    ptr_metadata,
)]

use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
};
use lzalloc::{vec::Vec, Result};
use spin::Mutex;

/// ## Remark
/// This align shouldn't be constant; it needs to be dynamic based upon:
/// * Cache line size
/// * Available memory
/// * Desired memory profile    
const SLAB_LENGTH: usize = 0x4000;

pub struct Slab<A: Allocator> {
    layout: Layout,
    capacity: usize,
    items: Option<Vec<NonNull<[u8]>, A>>,
    memory: NonNull<[u8]>,
    allocator: A,
}

impl<A: Allocator> core::fmt::Debug for Slab<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Slab")
            .field("Layout", &self.layout())
            .field("Capacity", &self.capacity())
            .field("Items", &self.items().len())
            .field("Memory", &self.memory.to_raw_parts())
            .finish()
    }
}

impl<A: Allocator + Copy> Slab<A> {
    fn new_in(layout: Layout, allocator: A) -> lzalloc::Result<Self> {
        let padded_layout = layout.pad_to_align();
        let memory = allocator.allocate(unsafe { Layout::from_size_align_unchecked(SLAB_LENGTH, layout.align()) })?;
        let capacity = memory.len() / padded_layout.size();
        let mut list = Vec::with_capacity_in(capacity, allocator).map_err(|_| AllocError)?;

        for index in 0..capacity {
            let start_offset = index * padded_layout.size();
            let end_offset = start_offset + layout.size();
            list.push(unsafe { memory.get_unchecked_mut(start_offset..end_offset) }).map_err(|_| AllocError)?;
        }

        Ok(Self { layout, capacity, items: Some(list), memory, allocator })
    }
}

impl<A: Allocator> Slab<A> {
    #[inline]
    const fn items(&self) -> &Vec<NonNull<[u8]>, A> {
        // ### Safety: `self.items` is only `None` when being dropped.
        unsafe { self.items.as_ref().unwrap_unchecked() }
    }

    #[inline]
    const fn items_mut(&mut self) -> &mut Vec<NonNull<[u8]>, A> {
        // ### Safety: `self.items` is only `None` when being dropped.
        unsafe { self.items.as_mut().unwrap_unchecked() }
    }

    #[inline]
    pub const fn layout(&self) -> Layout {
        self.layout
    }

    #[inline]
    pub const fn remaining(&self) -> usize {
        self.items().len()
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn take_item(&mut self) -> Option<NonNull<[u8]>> {
        log::trace!("TAKE ITEM");
        self.items_mut().pop()
    }

    pub fn return_item(&mut self, ptr: NonNull<u8>) -> Result<()> {
        if (unsafe { &*self.memory.as_ptr() }).as_ptr_range().contains(&ptr.as_ptr().cast_const())
            && ptr.addr().get().next_multiple_of(self.layout().align()) == ptr.addr().get()
        {
            let layout_size = self.layout().size();
            self.items_mut().push(NonNull::slice_from_raw_parts(ptr, layout_size)).map_err(|_| AllocError)
        } else {
            Err(AllocError)
        }
    }
}

impl<A: Allocator> Drop for Slab<A> {
    fn drop(&mut self) {
        drop(self.items.take());

        unsafe {
            self.allocator.deallocate(
                self.memory.as_non_null_ptr(),
                Layout::from_size_align_unchecked(self.memory.len(), self.layout.align()),
            )
        };
    }
}

pub struct SlabAllocator<A: Allocator> {
    slabs: Mutex<Vec<Slab<A>, A>>,
    max_size: usize,
    allocator: A,
}

// ### Safety: Type does not use thread-specific logic.
unsafe impl<A: Allocator + Copy> Send for SlabAllocator<A> {}
// ### Safety: Type's mutable conversions are synchronized via `spin::Mutex`.
unsafe impl<A: Allocator + Copy> Sync for SlabAllocator<A> {}

impl<A: Allocator + Copy> SlabAllocator<A> {
    #[inline]
    pub fn new_in(max_size_shift: u32, allocator: A) -> Self {
        Self { slabs: Mutex::new(Vec::new_in(allocator)), max_size: 1 << max_size_shift, allocator }
    }
}

unsafe impl<A: Allocator + Copy> Allocator for SlabAllocator<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>> {
        let padded_layout = layout.pad_to_align();
        if padded_layout.size() > self.max_size {
            self.allocator.allocate(layout)
        } else {
            let mut slabs = self.slabs.lock();
            let slab = match slabs.iter_mut().find(|slab| slab.remaining() > 0 && slab.layout() == layout) {
                Some(slab) => slab,
                None => {
                    slabs.push(Slab::new_in(layout, self.allocator)?).map_err(|_| AllocError)?;
                    unsafe { slabs.iter_mut().last().unwrap() }
                }
            };

            Ok(slab.take_item().unwrap())
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let padded_layout = layout.pad_to_align();
        if padded_layout.size() > self.max_size {
            self.allocator.deallocate(ptr, layout);
        } else {
            let mut slabs = self.slabs.lock();
            slabs
                .iter_mut()
                .filter(|slab| slab.remaining() < slab.capacity() && slab.layout() == layout)
                .find_map(|slab| slab.return_item(ptr).ok())
                .unwrap();
        }
    }
}
