mod mapper;
pub use mapper::*;

use crate::memory::PageAttributes;
use core::{alloc::Allocator, cmp::Ordering, num::NonZeroUsize, ops::Range, ptr::NonNull};
use libsys::{page_size, Address, Page, Virtual};
use try_alloc::vec::TryVec;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MmapFlags : u16 {
        const READ_EXECUTE  = 1 << 1;
        const READ_WRITE    = 1 << 2;
        const NOT_DEMAND    = 1 << 8;
    }
}

impl From<MmapFlags> for PageAttributes {
    fn from(flags: MmapFlags) -> Self {
        let mut attributes = PageAttributes::empty();

        // RW and RX are mutually exclusive, so always else-if the bit checks.
        if flags.contains(MmapFlags::READ_WRITE) {
            attributes.insert(PageAttributes::RW);
        } else if flags.contains(MmapFlags::READ_EXECUTE) {
            attributes.insert(PageAttributes::RX);
        } else {
            attributes.insert(PageAttributes::RO);
        }

        if !flags.contains(MmapFlags::NOT_DEMAND) {
            attributes.remove(PageAttributes::PRESENT);
            attributes.insert(PageAttributes::DEMAND);
        }

        attributes
    }
}

// TODO better error type for this class of functions
#[derive(Debug, Clone, Copy)]
pub struct Error;

#[repr(transparent)]
#[derive(Debug, Clone)]
struct Region(Range<usize>);

impl Eq for Region {}
impl PartialEq for Region {
    fn eq(&self, other: &Self) -> bool {
        self.0.contains(&other.0.start) || self.0.contains(&other.0.end)
    }
}

impl Ord for Region {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.0.start.cmp(&other.0.start), self.0.end.cmp(&other.0.end)) {
            (Ordering::Greater, Ordering::Greater) => Ordering::Less,
            (Ordering::Less, Ordering::Less) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for Region {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Region {
    pub fn new(start: Address<Page>, end: Address<Page>) -> Self {
        Self(start.get().get()..end.get().get())
    }

    pub fn start(&self) -> Address<Page> {
        Address::new_truncate(self.0.start)
    }

    pub fn end(&self) -> Address<Page> {
        Address::new_truncate(self.0.end)
    }

    pub fn size(&self) -> usize {
        self.0.len()
    }

    pub fn contains(&self, address: Address<Virtual>) -> bool {
        self.0.contains(&address.get())
    }
}

pub struct AddressSpace<A: Allocator + Clone> {
    free: TryVec<Region, A>,
    used: TryVec<Region, A>,
    mapper: Mapper,
}

impl<A: Allocator + Clone> AddressSpace<A> {
    pub fn new(size: NonZeroUsize, allocator: A) -> Result<Self, Error> {
        let mut free = TryVec::new_in(allocator.clone());
        free.push(Region(0..size.get())).map_err(|_| Error)?;

        Ok(Self {
            free,
            used: TryVec::new_in(allocator.clone()),
            mapper: unsafe {
                Mapper::new_unsafe(super::PageDepth::current(), super::new_kmapped_page_table().ok_or(Error)?)
            },
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

        let (found_index, found_region) = self
            .free
            .iter()
            .enumerate()
            .find_map(|(index, region)| (region.size() >= size).then_some((index, region.clone())))
            .ok_or(Error)?;

        let new_free_start = Address::new(found_region.start().get().get() + size).unwrap();
        let new_used = Region::new(found_region.start(), new_free_start);
        let new_free = Region::new(new_free_start, found_region.end());

        // Safety: Memory range was taken from the freelist, and so is guaranteed to be unused.
        let memory_ptr = unsafe { self.mmap_exact_impl(new_used.start(), new_used.end(), flags)? };

        // Replace old free region with updated bounds.
        self.free[found_index] = new_free;

        let sorted_index = self.used.binary_search(&new_used).expect("overlapping memory mapped regions detected");
        self.used.insert(sorted_index, new_used).map_err(|_| Error)?;

        Ok(memory_ptr)
    }

    /// Internal function taking exact address range parameters to map a region of memory.
    ///
    /// Safety
    ///
    /// This function has next to no safety checks, and so should only be called when it is
    /// known for certain that the provided memory range is valid for the mapping with the
    /// provided memory map flags.
    unsafe fn mmap_exact_impl(
        &mut self,
        start: Address<Page>,
        end: Address<Page>,
        flags: MmapFlags,
    ) -> Result<NonNull<[u8]>, Error> {
        let address_range = start.get().get()..end.get().get();
        // Finally, map all of the allocated pages in the virtual address space.
        for page_base in address_range.step_by(page_size().get()) {
            let page = Address::new(page_base).ok_or(Error)?;
            self.mapper.auto_map(page, PageAttributes::from(flags)).map_err(|_| Error)?;
        }

        NonNull::new(start.as_ptr())
            .map(|ptr| NonNull::slice_from_raw_parts(ptr, end.get().get() - start.get().get()))
            .ok_or(Error)
    }

    /// Attempts to map a page to a real frame, only if the [`PageAttributes::DEMAND`] bit is set.
    pub fn try_demand(&mut self, page: Address<Page>) -> Result<(), Error> {
        self.mapper
            .get_page_attributes(page)
            .filter(|attributes| attributes.contains(PageAttributes::DEMAND))
            .map(|mut attributes| {
                self.mapper
                    .auto_map(page, {
                        // remove demand bit ...
                        attributes.remove(PageAttributes::DEMAND);
                        // ... insert present bit ...
                        attributes.insert(PageAttributes::PRESENT);
                        // ... return attributes
                        attributes
                    })
                    .unwrap()
            })
            .ok_or(Error)
    }

    pub fn is_mmapped(&self, address: Address<Virtual>) -> bool {
        self.mapper.is_mapped(Address::new_truncate(address.get()), None)
    }

    /// Safety
    ///
    /// Caller must ensure that switching the currently active address space will not cause undefined behaviour.
    pub unsafe fn swap_into(&self) {
        self.mapper.swap_into()
    }
}
