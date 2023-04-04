pub mod mapper;

use crate::memory::{paging, paging::Attributes};
use alloc::vec::Vec;
use core::{alloc::Allocator, num::NonZeroUsize, ops::Range, ptr::NonNull};
use libsys::{page_size, Address, Page, Virtual};
use mapper::Mapper;

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    /// Indicates an allocation error occured in the backing allocator.
    AllocError,

    /// Indicates a malformed raw address was provided to an `Address` constructor.
    MalformedAddress,

    /// Indicates a provided address was not usable by the function.
    InvalidAddress,

    OverlappingAddress, 

    NotMapped(Address<Virtual>),

    /// Provides the error that occured within the internal `Mapper`.
    Paging(paging::Error),
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Paging(err) => Some(err),
            _ => None,
        }
    }
}

impl From<paging::Error> for Error {
    fn from(value: paging::Error) -> Self {
        match value {
            paging::Error::AllocError => Error::AllocError,
            paging::Error::NotMapped(addr) => Error::NotMapped(addr),
            paging::Error::HugePage => Error::Paging(paging::Error::HugePage),
        }
    }
}

crate::default_display_impl!(Error);
crate::err_result_type!(Error);

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MmapFlags : u16 {
        const READ_EXECUTE  = 1 << 1;
        const READ_WRITE    = 1 << 2;
        const NOT_DEMAND    = 1 << 8;
    }
}

impl From<MmapFlags> for Attributes {
    fn from(flags: MmapFlags) -> Self {
        let mut attributes = Attributes::empty();

        // RW and RX are mutually exclusive, so always else-if the bit checks.
        if flags.contains(MmapFlags::READ_WRITE) {
            attributes.insert(Attributes::RW);
        } else if flags.contains(MmapFlags::READ_EXECUTE) {
            attributes.insert(Attributes::RX);
        } else {
            attributes.insert(Attributes::RO);
        }

        if !flags.contains(MmapFlags::NOT_DEMAND) {
            attributes.remove(Attributes::PRESENT);
            attributes.insert(Attributes::DEMAND);
        }

        attributes
    }
}

pub struct AddressSpace<A: Allocator + Clone> {
    free: Vec<Range<usize>, A>,
    mapper: Mapper,
}

impl<A: Allocator + Clone> AddressSpace<A> {
    pub fn new(size: NonZeroUsize, allocator: A) -> Result<Self> {
        let mut free = Vec::new_in(allocator.clone());
        free.push(0..size.get());

        Ok(Self {
            free,

            // Safety: Mapper depth is known-valid (from current), and the mapped page table is
            //          promised-valid from the kernel itself.
            mapper: unsafe {
                Mapper::new_unsafe(
                    super::PageDepth::current(),
                    super::new_kmapped_page_table().ok_or(Error::AllocError)?,
                )
            },
        })
    }

    pub fn map(
        &mut self,
        address: Option<Address<Page>>,
        page_count: NonZeroUsize,
        flags: MmapFlags,
    ) -> Result<NonNull<[u8]>> {
        if let Some(address) = address {
            self.map_exact(address, page_count, flags)
        } else {
            self.map_auto(page_count, flags)
        }
    }

    fn map_auto(&mut self, page_count: NonZeroUsize, flags: MmapFlags) -> Result<NonNull<[u8]>> {
        let size = page_count.get() * page_size();

        let index = self.free.iter().position(|region| region.len() >= size).ok_or(Error::AllocError)?;
        let found_copy = self.free[index].clone();
        let new_free = found_copy.start..(found_copy.end - size);

        // Update the free region, or remove it if it's now empty.
        if new_free.len() > 0 {
            self.free[index] = new_free;
        } else {
            self.free.remove(index);
        }

        // Safety: Memory range was taken from the freelist, and so is guaranteed to be unused.
        Ok(unsafe { self.map_direct(Address::new(new_free.end).unwrap(), page_count, flags)? })
    }

    fn map_exact(
        &mut self,
        address: Address<Page>,
        page_count: NonZeroUsize,
        flags: MmapFlags,
    ) -> Result<NonNull<[u8]>> {
        let size = page_count.get() * page_size();
        let req_region_start = address.get().get();
        let req_region_end = req_region_start + size;

        let index = self
            .free
            
            .iter().try_find(|region| {
                use core::cmp::Ordering;

                match (region.contains(&req_region_start), region.contains(&req_region_end)) {
                    (true, true) => Ok(region),
                    (false, true) =>
                }
            })
            // We are going to insert, so if the region mapping doesn't exist, just fail fast.
            .map_err(|_| Error::InvalidAddress)?;

    }

    /// Internal function taking exact address range parameters to map a region of memory.
    ///
    /// ### Safety
    ///
    /// This function has next to no safety checks, and so should only be called when it is
    /// known for certain that the provided memory range is valid for the mapping with the
    /// provided memory map flags.
    unsafe fn map_direct(
        &mut self,
        address: Address<Page>,
        page_count: NonZeroUsize,
        flags: MmapFlags,
    ) -> Result<NonNull<[u8]>> {
        (0..page_count.get())
            .map(|offset| offset * page_size())
            .map(|offset_base| address.get().get() + offset_base)
            .map(|address| Address::new(address))
            .try_for_each(|page| {
                let page = page.ok_or(Error::MalformedAddress)?;
                self.mapper.auto_map(page, Attributes::from(flags)).map_err(Error::from)
            });

        Ok(NonNull::slice_from_raw_parts(NonNull::new(address.as_ptr()).unwrap(), page_count.get() * page_size()))
    }

    /// Attempts to map a page to a real frame, only if the [`PageAttributes::DEMAND`] bit is set.
    pub fn try_demand(&mut self, page: Address<Page>) -> Result<()> {
        self.mapper
            .get_page_attributes(page)
            .filter(|attributes| attributes.contains(Attributes::DEMAND))
            .ok_or(Error::NotMapped(page.get()))
            .and_then(|mut attributes| {
                self.mapper
                    .auto_map(page, {
                        // remove demand bit ...
                        attributes.remove(Attributes::DEMAND);
                        // ... insert present bit ...
                        attributes.insert(Attributes::PRESENT);
                        // ... return attributes
                        attributes
                    })
                    .map_err(Error::from)
            })
    }

    pub fn is_mmapped(&self, address: Address<Virtual>) -> bool {
        self.mapper.is_mapped(Address::new_truncate(address.get()), None)
    }

    pub fn with_mapper<T>(&self, with_fn: impl FnOnce(&Mapper) -> T) -> T {
        with_fn(&self.mapper)
    }

    pub unsafe fn with_mapper_mut<T>(&mut self, with_fn: impl FnOnce(&mut Mapper) -> T) -> T {
        with_fn(&mut self.mapper)
    }

    /// ### Safety
    ///
    /// Caller must ensure that switching the currently active address space will not cause undefined behaviour.
    pub unsafe fn swap_into(&self) {
        self.mapper.swap_into();
    }
}
