use core::{
    alloc::{AllocError, Allocator, Layout},
    num::NonZeroUsize,
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

        let result = self.regions.iter().enumerate().try_fold(Ok(0usize), |mut address, (index, region)| {
            let aligned_address = lzstd::align_up(
                // Safety: If `address` is `Err(_)`, it shouldn't be folding anymore.
                unsafe { address.unwrap_unchecked() },
                // Safety: `Layout` does not allow `0` for alignments.
                unsafe { NonZeroUsize::new_unchecked(layout.align()) },
            );

            Ok(address)
        });
    }
}
