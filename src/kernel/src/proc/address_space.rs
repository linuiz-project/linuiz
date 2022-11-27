use core::{
    alloc::{Allocator, Layout},
    num::NonZeroUsize,
    ops::ControlFlow,
    ptr::NonNull,
};

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

        let search = self.regions.iter().try_fold((0usize, 0usize), |(index, address), region| {
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

            Some((mut index, address)) => {
                let aligned_address = lzstd::align_up(address, layout_align);
                let aligned_padding = aligned_address - address;

                if aligned_padding > 0 {
                    if let Some(region) = self.regions.get_mut(index) && region.free {
                        region.len += aligned_padding;
                    } else {
                        self.regions.insert(index, Region { len: aligned_padding, free: true }).map_err(|_| Error)?;
                        index += 1;
                    }
                }

                let remaining_len = {
                    let region = self.regions.get_mut(index).ok_or(Error)?;
                    region.len -= aligned_padding;
                    region.free = false;

                    let len = region.len;
                    region.len = layout.size();
                    len - region.len
                };

                if remaining_len > 0 {
                    if let Some(region) = self.regions.get_mut(index + 1) && region.free {
                        region.len += remaining_len;
                    } else {
                        self.regions.insert(index + 1, Region { len: remaining_len, free: true }).map_err(|_| Error)?;
                    }
                }

                NonNull::new(aligned_address as *mut u8)
                    .map(|ptr| NonNull::slice_from_raw_parts(ptr, layout.size()))
                    .ok_or(Error)
            }
        }
    }
}
