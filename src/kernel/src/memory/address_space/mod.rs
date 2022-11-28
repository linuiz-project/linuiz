mod mapper;
pub use mapper::*;

use crate::{
    memory::{Mapper, PageAttributes, PhysicalAlloactor},
    PAGE_SIZE,
};
use alloc::collections::{BTreeMap, TryReserveError};
use core::{
    alloc::{AllocError, Allocator, Layout},
    num::NonZeroUsize,
    ops::ControlFlow,
    ptr::NonNull,
};
use lzstd::{Address, Page};
use spin::{Lazy, RwLock};
use try_alloc::vec::TryVec;
use uuid::Uuid;

static ADDRESS_SPACES: Lazy<BTreeMap<Uuid, RwLock<AddressSpace<PhysicalAlloactor>>, PhysicalAlloactor>> =
    Lazy::new(|| BTreeMap::new_in(&*super::PMM));

pub fn register() -> Result<Uuid, AllocError> {
    todo!()
}

pub fn get(uuid: &Uuid) -> Option<&'static RwLock<AddressSpace<PhysicalAlloactor>>> {
    ADDRESS_SPACES.get(uuid)
}

bitflags::bitflags! {
    pub struct MmapFlags : usize {
        const READ_ONLY = 1 << 0;
        const READ_WRITE = 1 << 1;
        const READ_EXECUTE = 1 << 2;
        const NO_DEMAND = 1 << 3;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Error;

#[derive(Debug, Clone, Copy)]
struct Region {
    len: usize,
    free: bool,
}

pub struct AddressSpace<A: Allocator + Clone> {
    regions: TryVec<Region, A>,
    allocator: A,
    mapper: Mapper,
}

impl<A: Allocator + Clone> AddressSpace<A> {
    pub fn new_in(size: usize, hhdm_ptr: NonNull<u8>, allocator: A) -> Result<Self, TryReserveError> {
        if size > 0 {
            let mut vec = TryVec::new_in(allocator.clone());
            vec.push(Region { len: size, free: true })?;

            Self { regions: vec, allocator, mapper: Mapper::new(depth, hhdm_address, None) }
        }
    }

    pub fn mmap(
        &mut self,
        address: Option<NonNull<u8>>,
        layout: Layout,
        flags: MmapFlags,
    ) -> Result<NonNull<[u8]>, Error> {
        let layout =
            Layout::from_size_align(layout.size(), core::cmp::max(layout.align(), PAGE_SIZE)).map_err(|_| Error)?;
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
                    if let Some(region) = self.regions.get_mut(index.saturating_add(1)) && region.free {
                        region.len += remaining_len;
                    } else {
                        self.regions.insert(index + 1, Region { len: remaining_len, free: true }).map_err(|_| Error)?;
                    }
                }

                // Set up paging attributes based on provided mmap flags.
                let mut attributes = {
                    if flags.contains(MmapFlags::READ_EXECUTE) {
                        PageAttributes::RX
                    } else if flags.contains(MmapFlags::READ_WRITE) {
                        PageAttributes::RW
                    } else if flags.contains(MmapFlags::READ_ONLY) {
                        PageAttributes::RO
                    } else {
                        PageAttributes::empty()
                    }
                };
                // Demand paging is the default, but optionally the user can specify front-loading
                // the physical page allocations.
                if !flags.contains(MmapFlags::NO_DEMAND) {
                    attributes.insert(PageAttributes::DEMAND);
                }
                // Finally, map all of the allocated pages in the virtual address space.
                for page_base in (aligned_address..(aligned_address + layout.size())).step_by(PAGE_SIZE) {
                    let page = Address::<Page>::from_u64(page_base as u64, None).ok_or(Error)?;
                    self.mapper.auto_map(page, attributes).map_err(|_| Error)?;
                }

                NonNull::new(aligned_address as *mut u8)
                    .map(|ptr| NonNull::slice_from_raw_parts(ptr, layout.size()))
                    .ok_or(Error)
            }
        }
    }
}
