use crate::memory::{
    mapper::Mapper,
    paging,
    paging::{PageDepth, TableEntryFlags},
};
use alloc::vec::Vec;
use core::{alloc::Allocator, num::NonZeroUsize, ops::Range, ptr::NonNull};
use libsys::{page_size, Address, Page, Virtual};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmapPermissions {
    ReadExecute,
    ReadWrite,
    ReadOnly,
}

impl From<MmapPermissions> for TableEntryFlags {
    fn from(permissions: MmapPermissions) -> Self {
        match permissions {
            MmapPermissions::ReadExecute => TableEntryFlags::RX,
            MmapPermissions::ReadWrite => TableEntryFlags::RW,
            MmapPermissions::ReadOnly => TableEntryFlags::RO,
        }
    }
}

pub const DEFAULT_USERSPACE_SIZE: NonZeroUsize = NonZeroUsize::new(1 << 47).unwrap();

pub struct AddressSpace<A: Allocator + Clone> {
    free: Vec<Range<usize>, A>,
    mapper: Mapper,
}

impl<A: Allocator + Clone> AddressSpace<A> {
    pub fn new(size: NonZeroUsize, mapper: Mapper, allocator: A) -> Self {
        let mut free = Vec::new_in(allocator);
        free.push(page_size()..size.get());

        Self { free, mapper }
    }

    pub fn new_userspace(allocator: A) -> Self {
        Self::new(
            DEFAULT_USERSPACE_SIZE,
            unsafe { Mapper::new_unsafe(PageDepth::current(), crate::memory::copy_kernel_page_table().unwrap()) },
            allocator,
        )
    }

    pub fn mmap(
        &mut self,
        address: Option<Address<Page>>,
        page_count: NonZeroUsize,
        lazy: bool,
        permissions: MmapPermissions,
    ) -> Result<NonNull<[u8]>> {
        if let Some(address) = address {
            self.map_exact(address, page_count, lazy, permissions)
        } else {
            self.map_any(page_count, lazy, permissions)
        }
    }

    #[cfg_attr(debug_assertions, inline(never))]
    fn map_any(&mut self, page_count: NonZeroUsize, lazy: bool, permissions: MmapPermissions) -> Result<NonNull<[u8]>> {
        let size = page_count.get() * page_size();

        let index = self.free.iter().position(|region| region.len() >= size).ok_or(Error::AllocError)?;
        let found_copy = self.free[index].clone();
        let new_free = (found_copy.start + size)..found_copy.end;

        // Update the free region, or remove it if it's now empty.
        if new_free.len() > 0 {
            self.free[index] = new_free.clone();
        } else {
            self.free.remove(index);
        }

        // Safety: Memory range was taken from the freelist, and so is guaranteed to be unused.
        Ok(unsafe { self.invoke_mapper(Address::new(new_free.start).unwrap(), page_count, lazy, permissions)? })
    }

    #[cfg_attr(debug_assertions, inline(never))]
    fn map_exact(
        &mut self,
        address: Address<Page>,
        page_count: NonZeroUsize,

        lazy: bool,
        permissions: MmapPermissions,
    ) -> Result<NonNull<[u8]>> {
        let size = page_count.get() * page_size();
        let req_region_start = address.get().get();
        let req_region_end = req_region_start + size;

        let index = self
            .free
            .iter()
            .enumerate()
            .find_map(|(index, region)| {
                let start_contained = region.contains(&req_region_start);
                let end_contained = region.contains(&req_region_end);

                (start_contained == end_contained).then_some(index)
            })
            // We are going to insert, so if the region mapping doesn't exist, just fail fast.
            .ok_or(Error::InvalidAddress)?;

        let found_copy = self.free[index].clone();
        let pre_range = found_copy.start..req_region_start;
        let post_range = req_region_end..found_copy.end;

        match (pre_range.len(), post_range.len()) {
            (0, 0) => {
                self.free.remove(index);
            }

            (0, _) => self.free[index] = post_range,
            (_, 0) => self.free[index] = pre_range,
            (_, _) => {
                self.free[index] = pre_range;
                self.free.insert(index + 1, post_range);
            }
        }

        unsafe { self.invoke_mapper(address, page_count, lazy, permissions) }
    }

    /// Internal function taking exact address range parameters to map a region of memory.
    ///
    /// ### Safety
    ///
    /// This function has next to no safety checks, and so should only be called when it is
    /// known for certain that the provided memory range is valid for the mapping with the
    /// provided memory map flags.
    unsafe fn invoke_mapper(
        &mut self,
        address: Address<Page>,
        page_count: NonZeroUsize,

        lazy: bool,
        permissions: MmapPermissions,
    ) -> Result<NonNull<[u8]>> {
        (0..page_count.get())
            .map(|offset| offset * page_size())
            .map(|offset| address.get().get() + offset)
            .map(|address| Address::new(address))
            .try_for_each(|page| {
                let page = page.ok_or(Error::MalformedAddress)?;
                let flags = TableEntryFlags::USER
                    | TableEntryFlags::from(permissions)
                    | if lazy { TableEntryFlags::DEMAND } else { TableEntryFlags::PRESENT };

                trace!("Invoking mapper: {:X?} {:?}", page, flags);
                self.mapper.auto_map(page, flags).map_err(Error::from)
            })
            .map(|_| {
                NonNull::slice_from_raw_parts(NonNull::new(address.as_ptr()).unwrap(), page_count.get() * page_size())
            })
    }

    /// Attempts to map a page to a real frame, only if the [`PageAttributes::DEMAND`] bit is set.
    pub fn try_demand(&mut self, page: Address<Page>) -> Result<()> {
        trace!("Attempting to demand map page: {:?}", page);

        self.mapper
            .get_page_attributes(page)
            .filter(|attributes| attributes.contains(TableEntryFlags::DEMAND))
            .ok_or(Error::NotMapped(page.get()))
            .and_then(|mut attributes| {
                self.mapper
                    .auto_map(page, {
                        // remove demand bit ...
                        attributes.remove(TableEntryFlags::DEMAND);
                        // ... insert present bit ...
                        attributes.insert(TableEntryFlags::PRESENT);
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
