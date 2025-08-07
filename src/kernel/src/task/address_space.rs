use crate::{
    mem::{
        KernelMapper, Permissions,
        mapper::{AutoMappingError, GetMappingError, Mapper},
    },
    task::asid::AddressSpaceId,
};
use core::{
    cmp::Ordering,
    num::NonZero,
    ops::{ControlFlow, Range},
    ptr::NonNull,
};
use libsys::{
    address::{Address, Page},
    constants::{page_bits, page_size},
};

pub enum MemoryMapping {
    Exact { range: Range<Address<Page>> },
    Any { count: NonZero<usize> },
}

#[derive(Debug)]
pub struct AddressSpace {
    id: AddressSpaceId,
    mapper: Mapper,
}

impl AddressSpace {
    pub fn new(id: AddressSpaceId) -> Self {
        Self {
            id,
            mapper: KernelMapper::with(Mapper::clone),
        }
    }

    pub fn is_current(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                let (current_table_address, _) =
                    crate::arch::x86_64::registers::control::cr3::CR3::read();

                current_table_address == self.mapper.root_table().frame()
            }
        }
    }

    pub fn mmap(
        &mut self,
        mapping: MemoryMapping,
        // TODO support lazy mapping
        // lazy: bool,
        permissions: Permissions,
    ) -> Result<NonNull<[u8]>, AutoMappingError> {
        match mapping {
            MemoryMapping::Exact { range } => self.map_range(range, permissions),
            MemoryMapping::Any { count } => self.map_any(count, permissions),
        }
    }

    /// Maps a range of addresses.
    fn map_range(
        &mut self,
        range: Range<Address<Page>>,
        permissions: Permissions,
    ) -> Result<NonNull<[u8]>, AutoMappingError> {
        range
            .clone()
            .try_for_each(|offset_page| self.mapper.auto_map(offset_page, permissions))?;

        let mapping_ptr = core::ptr::with_exposed_provenance_mut::<u8>(range.start.get().get());
        let mapping_len = range.count() * page_size();

        Ok(NonNull::slice_from_raw_parts(
            NonNull::new(mapping_ptr).unwrap(),
            mapping_len,
        ))
    }

    fn map_any(
        &mut self,
        count: NonZero<usize>,
        permissions: Permissions,
    ) -> Result<NonNull<[u8]>, AutoMappingError> {
        let mut start_index = 0;
        let mut run = 0;

        self.mapper
            .root_table()
            .walk(|entry| {
                if entry.is_none() {
                    run += 1;

                    if run == count.get() {
                        return ControlFlow::Break(());
                    }
                } else {
                    run = 0;
                }

                start_index += 1;

                ControlFlow::Continue(())
            })
            .break_value()
            .ok_or(AutoMappingError::OutOfMemory)?;

        match run.cmp(&count.get()) {
            Ordering::Equal => {
                let end_index = start_index + count.get();

                let start_page = Address::<Page>::new(start_index << page_bits().get()).unwrap();
                let end_page = Address::<Page>::new(end_index << page_bits().get()).unwrap();

                self.map_range(start_page..end_page, permissions)
            }

            Ordering::Less => Err(AutoMappingError::OutOfMemory),
            Ordering::Greater => unreachable!(),
        }
    }

    pub fn get_permissions(&self, page: Address<Page>) -> Result<Permissions, GetMappingError> {
        let permissions = self.mapper.get_permissions(page)?;

        Ok(permissions)
    }

    pub unsafe fn set_permissions(
        &mut self,
        page: Address<Page>,
        permissions: Permissions,
    ) -> Result<(), GetMappingError> {
        self.mapper.set_page_permissions(page, None, permissions)
    }

    pub fn is_mmapped(&self, address: Address<Page>) -> bool {
        self.mapper.is_mapped(address, None)
    }

    pub unsafe fn swap_into(&self) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            self.mapper.swap_into(&self.id);
        }
    }
}
