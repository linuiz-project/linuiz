use crate::mem::{
    mapper::Mapper,
    paging,
    paging::{TableDepth, TableEntryFlags},
};
use core::{num::NonZeroUsize, ptr::NonNull};
use libsys::{Address, Page, Virtual, page_size};

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    #[error("address space has run out of memory")]
    OutOfMemory,

    #[error("a malformed address address was provided")]
    MalformedAddress,

    #[error("a provided address was not usable by the function")]
    InvalidAddress,

    #[error("provided address range overruns valid virtual addresses")]
    AddressRangeOverrun,

    #[error("address is not mapped: {0:X?}")]
    NotMapped(Address<Virtual>),

    /// Provides the error that occured within the internal `Mapper`.
    #[error(transparent)]
    Mapper(#[from] paging::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
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
    #[inline]
    pub const fn new(mapper: Mapper) -> Self {
        Self(mapper)
    }

    pub fn new_userspace() -> Self {
        Self::new({
            // Safety: Kernel mapper is valid and has only one copy.
            unsafe {
                Mapper::new_unsafe(
                    TableDepth::max(),
                    crate::mem::copy_kernel_page_table().unwrap(),
                )
            }
        })
    }

    pub fn is_current(&self) -> bool {
        let root_frame = self.0.root_frame();
        let cr3_frame = crate::mem::PagingRegister::read().frame();

        root_frame == cr3_frame
    }

    // TODO maybe should return `Result<NonNull<[MaybeUninit<u8>]>>`?
    pub fn mmap(
        &mut self,
        address: Option<Address<Page>>,
        page_count: NonZeroUsize,
        // TODO support lazy mapping
        // lazy: bool,
        permissions: MmapPermissions,
    ) -> Result<NonNull<[u8]>, Error> {
        if let Some(address) = address {
            self.map_exact(address, page_count, permissions)
        } else {
            self.map_any(page_count, permissions)
        }
    }

    #[cfg_attr(debug_assertions, inline(never))]
    fn map_any(
        &mut self,
        _page_count: NonZeroUsize,
        _permissions: MmapPermissions,
    ) -> Result<NonNull<[u8]>, Error> {
        // let walker = unsafe {
        //     paging::walker::Walker::new(
        //         self.0.view_page_table(),
        //         TableDepth::max(),
        //         TableDepth::min(),
        //     )
        //     .unwrap()
        // };

        // let mut index = 0;
        // let mut run = 0;
        // walker.walk(|entry| {
        //     use core::ops::ControlFlow;

        //     if entry.is_none() {
        //         run += 1;

        //         if run == page_count.get() {
        //             return ControlFlow::Break(());
        //         }
        //     } else {
        //         run = 0;
        //     }

        //     index += 1;

        //     ControlFlow::Continue(())
        // });

        // match run.cmp(&page_count.get()) {
        //     core::cmp::Ordering::Equal => {
        //         let address = Address::<Page>::new(index << libsys::page_shift().get()).unwrap();
        //         let flags = TableEntryFlags::PRESENT
        //             | TableEntryFlags::USER
        //             | TableEntryFlags::from(permissions);

        //         unsafe { self.invoke_mapper(address, page_count, flags) }
        //     }
        //     core::cmp::Ordering::Less => Err(Error::OutOfMemory),
        //     core::cmp::Ordering::Greater => unreachable!(),
        // }

        todo!()
    }

    fn map_exact(
        &mut self,
        address: Address<Page>,
        page_count: NonZeroUsize,
        permissions: MmapPermissions,
    ) -> Result<NonNull<[u8]>, Error> {
        // Safety: Caller is required to maintain invariants.
        unsafe {
            self.invoke_mapper(
                address,
                page_count,
                TableEntryFlags::PRESENT
                    | TableEntryFlags::USER
                    | TableEntryFlags::from(permissions),
            )
        }
    }

    /// ## Safety
    ///
    /// Caller must ensure that mapping the provided page range, with the provided page flags, will not cause undefined behaviour.
    unsafe fn invoke_mapper(
        &mut self,
        address: Address<Page>,
        page_count: NonZeroUsize,
        flags: TableEntryFlags,
    ) -> Result<NonNull<[u8]>, Error> {
        let mapping_size = page_count.get() * page_size();
        (0..mapping_size)
            .step_by(page_size())
            .map(|offset| Address::new_truncate(address.get().get() + offset))
            .try_for_each(|offset_page| self.0.auto_map(offset_page, flags))?;

        Ok(NonNull::slice_from_raw_parts(
            NonNull::new(address.as_ptr()).unwrap(),
            mapping_size,
        ))
    }

    /// # Safety
    ///
    /// TODO
    pub unsafe fn set_flags(
        &mut self,
        address: Address<Page>,
        page_count: NonZeroUsize,
        flags: TableEntryFlags,
    ) -> Result<(), Error> {
        for index_offset in 0..page_count.get() {
            let offset_index = address.index() + index_offset;
            let offset_address =
                Address::from_index(offset_index).ok_or(Error::AddressRangeOverrun)?;

            // Safety: Caller is required to maintain safety invariants.
            unsafe {
                self.0.set_page_attributes(
                    offset_address,
                    None,
                    flags,
                    paging::FlagsModify::Set,
                )?;
            }
        }

        Ok(())
    }

    pub fn get_flags(&self, address: Address<Page>) -> Result<TableEntryFlags, Error> {
        self.0
            .get_page_attributes(address)
            .ok_or(Error::NotMapped(address.get()))
    }

    pub fn is_mmapped(&self, address: Address<Page>) -> bool {
        self.0.is_mapped(address, None)
    }

    /// # Safety
    ///
    /// Caller must ensure that switching the currently active address space will not cause undefined behaviour.
    pub unsafe fn swap_into(&self) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            self.0.swap_into();
        }
    }
}

impl core::fmt::Debug for AddressSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("AddressSpace")
            .field(&self.0.view_page_table().as_ptr())
            .finish()
    }
}
