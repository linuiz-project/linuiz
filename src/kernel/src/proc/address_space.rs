use core::{
    alloc::{AllocError, Allocator, Layout},
    num::NonZeroUsize,
    ops::ControlFlow,
    ptr::NonNull,
};

use alloc::collections::BTreeSet;
use try_alloc::vec::TryVec;

#[derive(Debug, Clone, Copy)]
pub struct Error;

#[derive(Debug, Clone, Copy)]
struct Region {
    len: usize,
    free: bool,
}

pub struct AddressSpace<A: Allocator + Clone> {
    min_alignment: usize,
    regions: TryVec<Region, A>,
    allocator: A,
}

impl<A: Allocator + Clone> AddressSpace<A> {
    pub fn mmap(&mut self, address: Option<NonNull<u8>>, layout: Layout) -> Result<NonNull<[u8]>, Error> {
        let layout = Layout::from_size_align(layout.size(), core::cmp::max(layout.align(), self.min_alignment))
            .map_err(|_| Error)?;
        // Safety: `Layout` does not allow `0` for alignments.
        let layout_align = unsafe { NonZeroUsize::new_unchecked(layout.align()) };

        let search = self.regions.iter().try_fold((0usize, 0usize), |(mut index, mut address), region| {
            let aligned_address = lzstd::align_up(address, layout_align);
            let aligned_padding = aligned_address - address;
            let aligned_len = region.len.saturating_sub(aligned_padding);

            if aligned_len < layout.size() {
                ControlFlow::Continue((index + 1, address + region.len))
            } else {
                ControlFlow::Break((index, address))
            }
        });

        match search.break_value() {
            None => Err(Error),

            Some((index, address)) => {
                let aligned_address = lzstd::align_up(address, layout_align);
                
            }
        }
    }
}
