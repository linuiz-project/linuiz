use crate::mem::{Permissions, mapper::Mapper};
use core::{num::NonZero, ptr::NonNull};
use libsys::{Address, Page, Virtual, page_size};

#[derive(Debug, Error, PartialEq, Eq)]
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

    #[error(transparent)]
    Mapper(#[from] crate::mem::mapper::Error),
}

pub const DEFAULT_USERSPACE_SIZE: NonZero<usize> = NonZero::<usize>::new(1 << 47).unwrap();

#[derive(Debug)]
pub struct AddressSpace(Mapper);

impl AddressSpace {
    pub fn new() -> Self {
        crate::mem::with_kernel_mapper(|kernel_mapper| Self(kernel_mapper.clone()))
    }

    pub fn is_current(&self) -> bool {
        let root_table_address = self.0.get_root_table_address();

        cfg_select! {
            target_arch = "x86_64" => {
                let (current_table_address, _) = crate::arch::x86_64::registers::control::CR3::read();

                current_table_address == root_table_address
            }
        }
    }

    // TODO maybe should return `Result<NonNull<[MaybeUninit<u8>]>>`?
    pub fn mmap(
        &mut self,
        address: Option<Address<Page>>,
        page_count: NonZero<usize>,
        // TODO support lazy mapping
        // lazy: bool,
        permissions: Permissions,
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
        _page_count: NonZero<usize>,
        _permissions: Permissions,
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
        page_count: NonZero<usize>,
        permissions: Permissions,
    ) -> Result<NonNull<[u8]>, Error> {
        unsafe { self.invoke_mapper(address, page_count, permissions) }
    }

    unsafe fn invoke_mapper(
        &mut self,
        address: Address<Page>,
        page_count: NonZero<usize>,
        permissions: Permissions,
    ) -> Result<NonNull<[u8]>, Error> {
        let mapping_size = page_count.get() * page_size();
        (0..mapping_size)
            .step_by(page_size())
            .map(|offset| Address::new_truncate(address.get().get() + offset))
            .try_for_each(|offset_page| self.0.auto_map(offset_page, permissions))?;

        Ok(NonNull::slice_from_raw_parts(
            NonNull::new(address.as_ptr()).unwrap(),
            mapping_size,
        ))
    }

    pub fn get_permissions(&self, address: Address<Page>) -> Result<Permissions, Error> {
        let permissions = self.0.get_permissions(address)?;

        Ok(permissions)
    }

    pub unsafe fn set_permissions(
        &mut self,
        address: Address<Page>,
        page_count: NonZero<usize>,
        permissions: Permissions,
    ) -> Result<(), Error> {
        for index_offset in 0..page_count.get() {
            let offset_index = address.index() + index_offset;
            let offset_address =
                Address::from_index(offset_index).ok_or(Error::AddressRangeOverrun)?;

            // Safety: Caller is required to maintain safety invariants.
            unsafe {
                self.0
                    .set_page_permissions(offset_address, None, permissions)?;
            }
        }

        Ok(())
    }

    pub fn is_mmapped(&self, address: Address<Page>) -> bool {
        self.0.is_mapped(address, None)
    }

    pub unsafe fn swap_into(&self) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            self.0.swap_into();
        }
    }
}
