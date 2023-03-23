mod mapper;
pub use mapper::*;

use crate::{
    interrupts::InterruptCell,
    memory::{PageAttributes, PhysicalAllocator},
};
use alloc::collections::BTreeMap;
use core::{alloc::Allocator, num::NonZeroUsize, ops::Range, ptr::NonNull};
use libsys::{page_size, Address, Page};
use spin::{Lazy, Mutex, RwLock};
use try_alloc::vec::TryVec;
use uuid::Uuid;

use super::Virtual;

static ADDRESS_SPACES: InterruptCell<
    Lazy<RwLock<BTreeMap<Uuid, Mutex<AddressSpace<PhysicalAllocator>>, PhysicalAllocator>>>,
> = InterruptCell::new(Lazy::new(|| RwLock::new(BTreeMap::new_in(&*super::PMM))));

pub fn register(uuid: Uuid, size: NonZeroUsize) -> Result<(), Error> {
    let address_space = unsafe { AddressSpace::new(size, &*super::PMM).map_err(|_| Error) }?;

    ADDRESS_SPACES.with(|address_spaces| {
        let mut guard = address_spaces.write();
        guard.try_insert(uuid, Mutex::new(address_space)).map(|_| ()).map_err(|_| Error)
    })
}

pub fn with<T>(uuid: &Uuid, func: impl FnOnce(&mut AddressSpace<PhysicalAllocator>) -> T) -> Option<T> {
    ADDRESS_SPACES.with(|address_spaces| {
        let address_spaces = address_spaces.read();
        address_spaces.get(uuid).map(|address_space| {
            let mut address_space = address_space.lock();
            func(&mut *address_space)
        })
    })
}

bitflags::bitflags! {
    pub struct MmapFlags : u16 {
        const READ = 0b1;
        const READ_WRITE = 0b11;
        const READ_EXECUTE = 0b111;
        const NOT_DEMAND = 0b1000;
    }
}

impl From<MmapFlags> for PageAttributes {
    fn from(flags: MmapFlags) -> Self {
        let mut attributes = PageAttributes::empty();

        if !flags.contains(MmapFlags::NOT_DEMAND) {
            attributes.insert(PageAttributes::DEMAND);
        }

        if flags.contains(MmapFlags::READ_EXECUTE) {
            attributes.insert(PageAttributes::RX);
        } else if flags.contains(MmapFlags::READ_WRITE) {
            attributes.insert(PageAttributes::RW);
        } else if flags.contains(MmapFlags::READ) {
            attributes.insert(PageAttributes::RO);
        }

        attributes
    }
}

impl From<PageAttributes> for MmapFlags {
    fn from(attributes: PageAttributes) -> Self {
        let mut flags = MmapFlags::empty();

        if !attributes.contains(PageAttributes::DEMAND) {
            flags.insert(MmapFlags::NOT_DEMAND)
        }

        if attributes.contains(PageAttributes::RX) {
            flags.insert(MmapFlags::READ_EXECUTE);
        } else if attributes.contains(PageAttributes::RW) {
            flags.insert(MmapFlags::READ_WRITE);
        } else if attributes.contains(PageAttributes::RO) {
            flags.insert(MmapFlags::READ);
        }

        flags
    }
}

// TODO better error type for this class of functions
#[derive(Debug, Clone, Copy)]
pub struct Error;

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct Region(Range<usize>);

impl Ord for Region {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.start.cmp(&other.0.start)
    }
}

impl PartialOrd for Region {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.0.start.partial_cmp(&other.0.start)
    }
}

pub struct AddressSpace<A: Allocator + Clone> {
    free: TryVec<Region, A>,
    used: TryVec<Region, A>,
    mapper: Mapper,
}

impl<A: Allocator + Clone> AddressSpace<A> {
    pub unsafe fn new(size: NonZeroUsize, allocator: A) -> Result<Self, Error> {
        let mut regions = TryVec::new_in(allocator.clone());
        regions.push(Region(0..size.get())).map_err(|_| Error)?;

        Ok(Self {
            free: regions,
            used: TryVec::new_in(allocator.clone()),
            mapper: Mapper::new(super::PageDepth::current()).ok_or(Error)?,
        })
    }

    pub fn mmap(
        &mut self,
        // TODO address: Option<Address<Page>>,
        pages: NonZeroUsize,
        flags: MmapFlags,
    ) -> Result<NonNull<[u8]>, Error> {
        let size = pages.get() * page_size().get();

        assert!(size.is_power_of_two());

        let search_index = self.free.iter().position(|region| region.0.len() >= size).ok_or(Error)?;

        let used_range = {
            let region = self.free.get_mut(search_index).unwrap();
            let used_region_start = region.0.start;
            let used_region_end = region.0.start + size;
            let free_region_end = region.0.end;
            // Update the free region's bounds to carve out the used region.
            region.0 = used_region_end..free_region_end;

            used_region_start..used_region_end
        };
        // Push a clone of the new used region to used list.
        self.used.push(Region(used_range.clone())).map_err(|_| Error)?;

        // Safety: Memory range was taken from the freelist, and so is guaranteed to be unused.
        unsafe { self.mmap_exact_impl(used_range, flags) }
    }

    /// Internal function taking exact address range parameters to map a region of memory.
    ///
    /// ### Safety
    ///
    /// This function has next to no safety checks, and so should only be called when it is
    /// known for certain that the provided memory range is valid for the mapping with the
    /// provided memory map flags.
    unsafe fn mmap_exact_impl(&mut self, range: Range<usize>, flags: MmapFlags) -> Result<NonNull<[u8]>, Error> {
        // Set up paging attributes based on provided mmap flags.
        let mut attributes = {
            if flags.contains(MmapFlags::READ_EXECUTE) {
                PageAttributes::RX
            } else if flags.contains(MmapFlags::READ_WRITE) {
                PageAttributes::RW
            } else if flags.contains(MmapFlags::READ) {
                PageAttributes::RO
            } else {
                PageAttributes::empty()
            }
        };

        // Demand paging is the default, but optionally the user can specify front-loading the physical page allocations.
        if !flags.contains(MmapFlags::NOT_DEMAND) {
            attributes.insert(PageAttributes::DEMAND);
        }

        // Finally, map all of the allocated pages in the virtual address space.
        for page_base in range.clone().step_by(page_size().get()) {
            let page = Address::new(page_base).ok_or(Error)?;
            self.mapper.auto_map(page, attributes).map_err(|_| Error)?;
        }

        NonNull::new(range.start as *mut u8).map(|ptr| NonNull::slice_from_raw_parts(ptr, range.len())).ok_or(Error)
    }

    /// Attempts to map a page to a real frame, only if the [`PageAttributes::DEMAND`] bit is set.
    pub fn try_map_demand_page(&mut self, page: Address<Page>) -> Result<(), Error> {
        match self.mapper.get_page_attributes(page) {
            Some(mut attributes) if attributes.contains(PageAttributes::DEMAND) => {
                self.mapper
                    .auto_map(page, {
                        // remove demand bit ...
                        attributes.remove(PageAttributes::DEMAND);
                        // ... insert present bit ...
                        attributes.insert(PageAttributes::PRESENT);
                        // ... return attributes
                        attributes
                    })
                    .unwrap();

                Ok(())
            }

            _ => Err(Error),
        }
    }

    pub fn is_mmapped(&self, address: Address<Virtual>) -> bool {
        self.mapper.is_mapped(Address::new_truncate(address.get()), None)
    }
}
