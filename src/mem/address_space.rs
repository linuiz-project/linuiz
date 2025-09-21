use crate::mem::{
    KernelMapper, Permissions,
    addr::{
        phys::FrameAddress,
        virt::{PageAddress, StandardPage, VirtualAddress},
    },
    mapper::{AutoMappingError, GetMappingError, Mapper},
};
use core::{num::NonZero, ops::Range, ptr::NonNull};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressSpaceId(usize);

impl AddressSpaceId {
    pub const MAX: usize = {
        cfg_select! {
            target_arch = "x86_64" => { 0b1111_1111_1111 }
            _ => { unimplemented!() }
        }
    };

    pub const KERNEL: Self = Self(0);

    pub fn new(id: usize) -> Option<Self> {
        (id <= Self::MAX).then_some(Self(id))
    }

    /// # Safety
    ///
    /// - `id` must be ≤`Self::MAX`.
    pub unsafe fn new_unchecked(id: usize) -> Self {
        Self(id)
    }
}

impl From<AddressSpaceId> for usize {
    fn from(value: AddressSpaceId) -> Self {
        value.0
    }
}

#[derive(Debug)]
pub enum MemoryMapping {
    Exact {
        range: Range<StandardPage>,
        invalidate_pages: bool,
    },
    Any {
        count: NonZero<usize>,
    },
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
            mapper: KernelMapper::clone(),
        }
    }

    pub fn is_current(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                let (current_table_address, _) =
                    crate::arch::x86_64::registers::control::cr3::CR3::read();

                current_table_address == self.mapper.frame()
            }
        }
    }

    /// Memory maps a range of memory.
    ///
    /// # Safety
    ///
    /// - If `mapping` is [`MemoryMapping::Exact`], it must reference only
    ///   currently unused pages.
    /// - `permissions` must be the correct permissions for how the memory will
    ///   be used.
    pub unsafe fn mmap(
        &mut self,
        mapping: MemoryMapping,
        // TODO support lazy mapping
        // lazy: bool,
        permissions: Permissions,
    ) -> Result<NonNull<[u8]>, AutoMappingError> {
        match mapping {
            MemoryMapping::Exact {
                range,
                invalidate_pages,
            } => {
                // Safety: Caller is required to maintain safety invariants.
                unsafe { self.map_range(range, permissions, invalidate_pages) }
            }

            MemoryMapping::Any { count: _ } => {
                #[allow(clippy::todo)]
                {
                    todo!()
                }
                // // Safety: Caller is required to maintain safety invariants.
                // unsafe { self.map_any(count, permissions) }
            }
        }
    }

    /// Maps a range of addresses.
    ///
    /// # Safety
    ///
    /// - `range` must not contain any pages that are already mapped.
    /// - `permissions` must be the correct permissions for how the memory will
    ///   be used.
    unsafe fn map_range(
        &mut self,
        range: Range<StandardPage>,
        permissions: Permissions,
        invalidate_pages: bool,
    ) -> Result<NonNull<[u8]>, AutoMappingError> {
        range.clone().try_for_each(|offset_page| {
            // Safety: Caller is required to maintain safety invariants.
            unsafe {
                self.mapper
                    .auto_map(offset_page, permissions, invalidate_pages)
            }
        })?;

        let mapping_address = NonZero::<usize>::try_from(usize::from(range.start)).unwrap();
        let mapping_ptr = NonNull::<u8>::with_exposed_provenance(mapping_address);
        let mapping_len = range.count() * <StandardPage as PageAddress>::Frame::SIZE_IN_BYTES.get();

        Ok(NonNull::slice_from_raw_parts(mapping_ptr, mapping_len))
    }

    // /// Maps a range of `page_count` length in pages.
    // ///
    // /// # Safety
    // ///
    // /// - `permissions` must be the correct permissions for how the memory will
    // be ///   used.
    // #[rustfmt::skip] // remove this when the function is fixed
    // unsafe fn map_any(
    //     &mut self,
    //     page_count: NonZero<usize>,
    //     permissions: Permissions,
    // ) -> Result<NonNull<[u8]>, AutoMappingError> {
    //     let mut start_index = 0;
    //     let mut run = 0;

    //     self.mapper
    //         .walk_all(|depth, page_table_index, entry| {

    //             if entry.is_none() {
    //                 run += 1;

    //                 if run == page_count.get() {
    //                     return ControlFlow::Break(());
    //                 }
    //             } else {
    //                 run = 0;
    //             }

    //             start_index += 1;

    //             ControlFlow::Continue(())
    //         })
    //         .break_value()
    //         .ok_or(AutoMappingError::OutOfMemory)?;

    //     match run.cmp(&page_count.get()) {
    //         Ordering::Equal => {
    //             let end_index = start_index + page_count.get();

    //             let start_page = Address::<Page>::new(start_index <<
    // page_bits().get()).unwrap();             let end_page =
    // Address::<Page>::new(end_index << page_bits().get()).unwrap();

    //             // Safety:
    //             // - Range is checked to be unused.
    //             // - Caller is required to ensure permissions are correct.
    //             unsafe { self.map_range(start_page..end_page, permissions) }
    //         }

    //         Ordering::Less => Err(AutoMappingError::OutOfMemory),
    //         Ordering::Greater => unreachable!(),
    //     }
    // }

    pub fn get_permissions(&self, address: VirtualAddress) -> Result<Permissions, GetMappingError> {
        self.mapper.get_permissions(address)
    }

    pub unsafe fn set_permissions(
        &mut self,
        address: VirtualAddress,
        permissions: Permissions,
    ) -> Result<(), GetMappingError> {
        // Safety: Caller is required to maintain safety invariants.
        unsafe { self.mapper.set_page_permissions(address, None, permissions) }
    }

    pub fn is_mmapped(&self, address: VirtualAddress) -> bool {
        self.mapper.is_mapped(address)
    }

    pub unsafe fn swap_into(&self) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            self.mapper.swap_into(self.id);
        }
    }
}
