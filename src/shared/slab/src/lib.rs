#![no_std]
#![feature(
    allocator_api,          // #32838 <https://github.com/rust-lang/rust/issues/32838>
    strict_provenance,      // #95228 <https://github.com/rust-lang/rust/issues/95228>
    pointer_is_aligned,     // #96284 <https://github.com/rust-lang/rust/issues/96284>
    ptr_as_uninit,          // #75402 <https://github.com/rust-lang/rust/issues/75402>
    slice_ptr_get,          // #74265 <https://github.com/rust-lang/rust/issues/74265>
    maybe_uninit_slice,     // #63569 <https://github.com/rust-lang/rust/issues/63569>
    int_roundings,          // #88581 <https://github.com/rust-lang/rust/issues/88581>
    const_cmp,              // N/A
)]

use bitvec::slice::BitSlice;
use core::{
    alloc::{AllocError, Allocator, Layout},
    mem::{align_of, size_of, MaybeUninit},
    ptr::NonNull,
};
use lzalloc::{vec::Vec, Result};

/// Type used as the backing store the [`bitvec::BitSlice`] allocation table.
/// Using this, the backing store type can be more easily modulated in cases
/// where performance is preferred over the space efficiency of the slabs.
/// notation: lomem feature
type SlabInt = u8;

/// ## Remark
/// This align shouldn't be constant; it needs to be dynamic based upon:
/// * Cache line size
/// * Available memory
/// * Desired memory profile    
const SLAB_LENGTH: usize = 0x1000;

pub struct Slab<'a> {
    layout: Layout,
    allocations: &'a mut BitSlice<SlabInt>,
    elements: NonNull<[u8]>,
}

impl<'a> Slab<'a> {
    fn new(ptr: NonNull<[u8]>, element_layout: Layout) -> Option<&'a mut Self> {
        // Ensure the pointer is aligned and the memory is the correct size for a slab.
        // ### Remark: The check for length may become less useful when dynamic slab sizing is added.
        if ptr.len() != SLAB_LENGTH || !ptr.as_ptr().is_aligned_to(align_of::<Self>()) {
            return None;
        }

        let header_size = size_of::<Self>();
        // We'd like to separate the elements by the maximum of their alignment or their size to the next power of two.
        let element_size = core::cmp::max(element_layout.size().next_power_of_two(), element_layout.align());

        // Reduce the allocation table sizing issue to `(slab_size + 1 bit) * 8` to calculate the suitable allocation table entries.
        // This is specifically an issue because the total size of the allocation is dynamic and dependent on the count of elements,
        // while the total count of elements is also constrained by the size of the allocation table (since they both take up bytes
        // in the resulting memory region).
        let allocations_len = {
            let grouping_size = (element_size * (SlabInt::BITS as usize)) + size_of::<SlabInt>();
            let slabs_length = ptr.len() - header_size;
            slabs_length / grouping_size
        };
        // ### Safety: Memory range is guaranteed to be within `ptr` address space due to the way `allocations_len` is calcualted.
        let allocations_ptr = unsafe { ptr.get_unchecked_mut(header_size..(header_size + allocations_len)) };

        let elements_offset = (header_size + allocations_len).next_multiple_of(element_layout.align());
        let elements_len = allocations_len * (SlabInt::BITS as usize);
        // ### Safety: Memory range is checked to be within `ptr` address space.
        let elements_ptr = unsafe { ptr.get_unchecked_mut(elements_offset..(elements_offset + elements_len)) };

        // Check to ensure the `elements_ptr` memory range doesn't go out bounds.
        debug_assert!(
            ptr.as_non_null_ptr().addr().checked_add(ptr.len())
                >= elements_ptr.as_non_null_ptr().addr().checked_add(elements_len)
        );

        Some({
            // ### Safety: Pointer is required to be valid for `Self`.
            let mut slab = unsafe { ptr.as_non_null_ptr().cast::<Self>().as_mut() };
            slab.layout = element_layout;
            // ### Safety
            // * Slab is checked valid for the given memory range.
            // * Allocation slice is of a known and correct length for `usize`.
            // * Slab is checked to contain no more than `BitSlice::MAX_BITS`.
            slab.allocations = unsafe {
                BitSlice::from_slice_unchecked_mut(MaybeUninit::slice_assume_init_mut(
                    allocations_ptr.as_uninit_slice_mut(),
                ))
            };
            // ### Safety: Memory range is calculated valid for the provided pointer and length.
            slab.elements = elements_ptr;

            slab
        })
    }

    #[inline]
    pub const fn layout(&self) -> Layout {
        self.layout
    }

    #[inline]
    pub fn allocations(&mut self) -> &mut BitSlice<SlabInt> {
        &mut self.allocations
    }

    #[inline]
    const fn element_size(&self) -> usize {
        core::cmp::max(self.layout().size().next_power_of_two(), self.layout().align())
    }

    pub fn get_next_ptr(&mut self) -> Option<NonNull<[u8]>> {
        let index = self.allocations().first_zero()?;
        let element_size = self.element_size();
        let offset = index * element_size;

        Some(unsafe { self.elements.get_unchecked_mut(offset..(offset + element_size)) })
    }
}

pub struct SlabAllocator<'a, A: Allocator> {
    slabs: Vec<&'a mut Slab<'a>, A>,
}

impl<A: Allocator> SlabAllocator<'_, A> {
    #[inline]
    pub fn new(allocator: A) -> Self {
        Self { slabs: Vec::new_in(allocator) }
    }

    pub fn get_object<T>(&mut self) -> Result<&mut MaybeUninit<T>> {
        let layout = Layout::new::<T>();
        let slab_size = core::cmp::max(layout.size(), layout.align());

        loop {
            match self
                .slabs
                .iter_mut()
                .find(|slab| slab.layout().size() == slab_size)
                .and_then(|slab| slab.get_next_ptr())
            {
                Some(element) => break Ok(unsafe { element.as_non_null_ptr().cast::<T>().as_uninit_mut() }),

                None => {
                    // Attempt to allocate a new slab to be used for this allocation.
                    if self
                        .slabs
                        .allocator()
                        // ### Safety: Layout parameters provided are valid.
                        .allocate_zeroed(unsafe { Layout::from_size_align_unchecked(SLAB_LENGTH, align_of::<Slab>()) })
                        .ok()
                        // Initialize the slab memory.
                        .and_then(|allocation| Slab::new(allocation, Layout::new::<T>()))
                        // Push the new slab to the list.
                        .and_then(|slab| self.slabs.push(slab).ok())
                        // Check if any of the previous operations failed.
                        .is_none()
                    {
                        return Err(AllocError);
                    }
                }
            }
        }
    }
}
