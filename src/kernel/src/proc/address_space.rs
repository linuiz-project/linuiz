use crate::memory::{
    mapper::Mapper,
    paging,
    paging::{PageDepth, TableEntryFlags},
};
use core::{num::NonZeroUsize, ptr::NonNull};
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

    AddressOverrun,

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

pub struct AddressSpace(Mapper);

impl AddressSpace {
    pub fn new(mapper: Mapper) -> Self {
        Self(mapper)
    }

    pub fn new_userspace() -> Self {
        Self::new(unsafe { Mapper::new_unsafe(PageDepth::current(), crate::memory::copy_kernel_page_table().unwrap()) })
    }

    pub fn mmap(
        &mut self,
        address: Option<Address<Page>>,
        page_count: NonZeroUsize,
        // TODO support lazy mapping
        // lazy: bool,
        permissions: MmapPermissions,
    ) -> Result<NonNull<[u8]>> {
        if let Some(address) = address {
            self.map_exact(address, page_count, permissions)
        } else {
            self.map_any(page_count, permissions)
        }
    }

    #[cfg_attr(debug_assertions, inline(never))]
    fn map_any(&mut self, page_count: NonZeroUsize, permissions: MmapPermissions) -> Result<NonNull<[u8]>> {
        let walker = unsafe {
            paging::walker::Walker::new(self.0.view_page_table(), PageDepth::current(), PageDepth::new(1).unwrap())
                .unwrap()
        };

        let mut index = 0;
        let mut run = 0;
        walker.walk(|entry| {
            use core::ops::ControlFlow;

            if entry.is_none() {
                run += 1;

                if run == page_count.get() {
                    return ControlFlow::Break(());
                }
            } else {
                run = 0;
            }

            index += 1;

            ControlFlow::Continue(())
        });

        match run.cmp(&page_count.get()) {
            core::cmp::Ordering::Equal => {
                let address = Address::<Page>::new(index << libsys::page_shift().get()).unwrap();
                let flags = TableEntryFlags::PRESENT | TableEntryFlags::USER | TableEntryFlags::from(permissions);

                unsafe { self.invoke_mapper(address, page_count, flags) }
            }
            core::cmp::Ordering::Less => Err(Error::AllocError),
            core::cmp::Ordering::Greater => unreachable!(),
        }
    }

    #[cfg_attr(debug_assertions, inline(never))]
    fn map_exact(
        &mut self,
        address: Address<Page>,
        page_count: NonZeroUsize,
        permissions: MmapPermissions,
    ) -> Result<NonNull<[u8]>> {
        let size = page_count.get() * page_size();
        let _end_address = Address::<Page>::new(address.get().get() + size).ok_or(Error::AddressOverrun);

        unsafe {
            self.invoke_mapper(
                address,
                page_count,
                TableEntryFlags::PRESENT | TableEntryFlags::USER | TableEntryFlags::from(permissions),
            )
        }
    }

    /// ### Safety
    ///
    /// Caller must ensure that mapping the provided page range, with the provided page flags, will not cause undefined behaviour.
    unsafe fn invoke_mapper(
        &mut self,
        address: Address<Page>,
        page_count: NonZeroUsize,
        flags: TableEntryFlags,
    ) -> Result<NonNull<[u8]>> {
        let mapping_size = page_count.get() * page_size();
        (0..mapping_size)
            .step_by(page_size())
            .map(|offset| Address::new_truncate(address.get().get() + offset))
            .try_for_each(|offset_page| self.0.auto_map(offset_page, flags))
            .map_err(Error::from)?;

        Ok(NonNull::slice_from_raw_parts(NonNull::new(address.as_ptr()).unwrap(), mapping_size))
    }

    pub unsafe fn set_flags(
        &mut self,
        address: Address<Page>,
        page_count: NonZeroUsize,
        flags: TableEntryFlags,
    ) -> Result<()> {
        let size = page_count.get() * page_size();
        let _end_address = Address::<Page>::new(address.get().get() + size).ok_or(Error::AddressOverrun);

        (0..size)
            .map(|offset| Address::new_truncate(address.get().get() + offset))
            .try_for_each(|offset_page| self.0.set_page_attributes(offset_page, None, flags, paging::FlagsModify::Set))
            .map_err(Error::from)
    }

    pub fn is_mmapped(&self, address: Address<Page>) -> bool {
        self.0.is_mapped(address, None)
    }

    /// ### Safety
    ///
    /// Caller must ensure that switching the currently active address space will not cause undefined behaviour.
    pub unsafe fn swap_into(&self) {
        self.0.swap_into();
    }
}

impl core::fmt::Debug for AddressSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("AddressSpace").field(&self.0.view_page_table().as_ptr()).finish()
    }
}
