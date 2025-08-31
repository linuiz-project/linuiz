use core::ops::ControlFlow;

use crate::{
    mem::{
        HigherHalfDirectMap, Permissions,
        mapper::paging::{Depth, Entry},
        pmm::{FrameError, NextFrameError, PhysicalMemoryManager},
    },
    task::asid::AddressSpaceId,
    util::{ExclusiveBorrow, SharedBorrow},
};
use libsys::address::{Address, Frame, Page};

pub mod paging;
use paging::{CreateEntryError, PageTable, WithEntryError};

/// Whether the current environment supports 2MiB pages.
pub fn use_large_pages() -> bool {
    cfg_select! {
        target_arch = "x86_64" => {
            use crate::arch::x86_64::{cpuid::feature_info, registers::control::cr4};

            debug_assert!(
                feature_info().is_some_and(|cpuid| cpuid.has_pae())
                    && cr4::CR4::read().contains(cr4::Flags::PAE)
            );

            true
        }

        _ => { unimplemented!() }
    }
}

/// Whether the current environment supports 1GiB pages.
pub fn use_huge_pages() -> bool {
    cfg_select! {
        target_arch = "x86_64" => {
            crate::arch::x86_64::cpuid::extended_feature_identifiers()
                .is_some_and(|cpuid| cpuid.has_1gib_pages())
        }

        _ => { unimplemented!() }
    }
}

#[derive(Debug, Error)]
pub enum MappingError {
    #[error("attempted mapping to location outside memory")]
    OutsideMemory,

    #[error("attempted to map an already mapped region")]
    AlreadyMapped,

    #[error("ran out of memory for allocation")]
    OutOfMemory,
}

impl From<FrameError> for MappingError {
    fn from(error: FrameError) -> Self {
        match error {
            FrameError::OutOfBounds => Self::OutsideMemory,
        }
    }
}

impl From<CreateEntryError> for MappingError {
    fn from(error: CreateEntryError) -> Self {
        match error {
            CreateEntryError::TerminatingPage => Self::AlreadyMapped,
            CreateEntryError::OutOfMemory => Self::OutOfMemory,
        }
    }
}

#[derive(Debug, Error)]
pub enum UnmappingError {
    #[error("page was not mapped")]
    NotMapped,
}

impl From<FrameError> for UnmappingError {
    fn from(error: FrameError) -> Self {
        match error {
            FrameError::OutOfBounds => Self::NotMapped,
        }
    }
}

impl From<WithEntryError> for UnmappingError {
    fn from(error: WithEntryError) -> Self {
        match error {
            WithEntryError::TerminatingPage | WithEntryError::NotMapped => Self::NotMapped,
        }
    }
}

#[derive(Debug, Error)]
pub enum AutoMappingError {
    #[error("ran out of memory for allocation")]
    OutOfMemory,
}

impl From<NextFrameError> for AutoMappingError {
    fn from(error: NextFrameError) -> Self {
        match error {
            NextFrameError::NoneFree => Self::OutOfMemory,
        }
    }
}

impl From<MappingError> for AutoMappingError {
    fn from(error: MappingError) -> Self {
        match error {
            MappingError::OutsideMemory | MappingError::AlreadyMapped => unreachable!(),
            MappingError::OutOfMemory => Self::OutOfMemory,
        }
    }
}

#[derive(Debug, Error)]
pub enum GetMappingError {
    #[error("page was not mapped")]
    NotMapped,
}

impl From<WithEntryError> for GetMappingError {
    fn from(error: WithEntryError) -> Self {
        match error {
            WithEntryError::TerminatingPage | WithEntryError::NotMapped => Self::NotMapped,
        }
    }
}

#[derive(Clone)]
pub struct Mapper(Address<Frame>);

impl Mapper {
    pub fn new() -> Self {
        let frame = PhysicalMemoryManager::next_free(core::num::NonZero::<usize>::MIN, true)
            .expect("failed to allocate frame for new root page table");

        Self(frame)
    }

    pub fn frame(&self) -> Address<Frame> {
        self.0
    }

    fn root_table(&self) -> PageTable<SharedBorrow> {
        // Safety:
        // - `Self::new` always allocates a cleared frame.
        // - `Depth::min` is the current depth.
        unsafe { PageTable::<SharedBorrow>::new(self.frame(), Depth::min()) }
    }

    fn root_table_mut(&mut self) -> PageTable<ExclusiveBorrow> {
        // Safety:
        // - `Self::new` always allocates a cleared frame.
        // - `Depth::min` is the current depth.
        unsafe { PageTable::<ExclusiveBorrow>::new(self.frame(), Depth::min()) }
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the frame.
    ///
    /// # Safety
    ///
    /// - `frame` must not be currently mapped by another virtual memory
    ///   mapping.
    /// - `depth` must be a supported page table mapping depth.
    /// - `memory_access` must be the correct memory access permissions for the
    ///   mapping (i.e. mapping a `.bss` section as read-only would cause a
    ///   `#PF`).
    pub unsafe fn map(
        &mut self,
        page: Address<Page>,
        frame: Address<Frame>,
        depth: Depth,
        lock_frame: bool,
        permissions: Permissions,
    ) -> Result<(), MappingError> {
        // TODO: Check that `depth` is a supported mapping depth.

        trace!(
            "Mapping: {:#X} -> {:#X} {{ Depth: {}, {permissions:?}, Lock: {lock_frame} }}",
            page.get().get(),
            frame.get().get(),
            depth.get()
        );

        if lock_frame {
            PhysicalMemoryManager::lock_frame(frame)?;
        }

        // If acquisition of the frame is successful, attempt to map the page to the
        // frame index.
        self.root_table_mut()
            .with_entry_create(page, depth, |entry| {
                #[cfg(target_arch = "x86_64")]
                if depth > Depth::max() {
                    entry.set_huge(true);
                }

                if HigherHalfDirectMap::is_address_higher_half(page.get()) {
                    entry.set_global(true);
                } else {
                    entry.set_user(true);
                }

                // Safety: Caller is required to maintain safety invariants.
                unsafe {
                    entry.set_frame(frame);
                }

                // Safety: Caller is required to maintain safety invariants.
                unsafe {
                    entry.set_permissions(permissions);
                }

                // Safety: Caller is required to ensure `frame` is not in use.
                unsafe {
                    entry.set_enabled(true);
                }

                #[cfg(target_arch = "x86_64")]
                crate::arch::x86_64::instructions::__invlpg(page);

                trace!("Mapped: {entry:X?}");
            })?;

        Ok(())
    }

    /// Unmaps the given page, optionally freeing the frame the page points to
    /// within the given [`FrameManager`].
    ///
    /// # Safety
    ///
    /// Caller must ensure calling this function does not cause memory
    /// corruption.
    pub unsafe fn unmap(
        &mut self,
        page: Address<Page>,
        to_depth: Option<Depth>,
        free_frame: bool,
    ) -> Result<(), UnmappingError> {
        self.root_table_mut()
            .with_entry_mut(page, to_depth, |entry| {
                let frame = entry.get_frame().ok_or(UnmappingError::NotMapped)?;

                // Safety: Caller is required to maintain invariants.
                unsafe {
                    entry.clear();
                }

                if free_frame {
                    PhysicalMemoryManager::free_frame(frame)?;
                }

                // Invalidate the page in the TLB.
                #[cfg(target_arch = "x86_64")]
                crate::arch::x86_64::instructions::__invlpg(page);

                Ok(())
            })?
    }

    /// Maps the specified page to an automatically allocated frame.
    ///
    /// # Safety
    ///
    /// - `page` must not be currently mapped to another physical address.
    /// - `permissions` must be the correct permissions for how the memory will
    ///   be used.
    pub unsafe fn auto_map(
        &mut self,
        page: Address<Page>,
        permissions: Permissions,
    ) -> Result<(), AutoMappingError> {
        let frame = PhysicalMemoryManager::next_free(core::num::NonZero::<usize>::MIN, true)?;

        // Safety:
        // - Frame was just allocated, so not current in use.
        // - Depth is the maximum paging depth, which is always supported.
        // - Caller is required to ensure permissions are correct.
        unsafe {
            self.map(page, frame, Depth::max(), false, permissions)?;
        }

        Ok(())
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: Address<Page>, depth: Option<Depth>) -> bool {
        self.root_table().with_entry(page, depth, |_| ()).is_ok()
    }

    pub fn is_mapped_to(&self, page: Address<Page>, frame: Address<Frame>) -> bool {
        self.root_table()
            .with_entry(page, None, |entry| {
                entry
                    .get_frame()
                    .is_some_and(|entry_frame| entry_frame == frame)
            })
            .unwrap_or(false)
    }

    pub fn get_mapped_to(&self, page: Address<Page>) -> Result<Address<Frame>, GetMappingError> {
        self.root_table()
            .with_entry(page, None, |entry| {
                entry.get_frame().ok_or(GetMappingError::NotMapped)
            })
            .map_err(|error| match error {
                super::mapper::WithEntryError::NotMapped => GetMappingError::NotMapped,
                super::mapper::WithEntryError::TerminatingPage => unreachable!(),
            })
            .flatten()
    }

    /* STATE CHANGING */

    pub fn get_permissions(&self, page: Address<Page>) -> Result<Permissions, GetMappingError> {
        let permissions = self
            .root_table()
            .with_entry(page, None, Entry::get_permissions)?;

        Ok(permissions)
    }

    pub unsafe fn set_page_permissions(
        &mut self,
        page: Address<Page>,
        depth: Option<Depth>,
        permissions: Permissions,
    ) -> Result<(), GetMappingError> {
        self.root_table_mut().with_entry_mut(page, depth, |entry| {
            // Safety: Caller is required to maintain safety invariants.
            unsafe {
                entry.set_permissions(permissions);
            }

            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::instructions::__invlpg(page);
        })?;

        Ok(())
    }

    pub unsafe fn swap_into(&self, address_space_id: AddressSpaceId) {
        let root_table_frame = self.frame();

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            cfg_select! {
               target_arch = "x86_64" => {
                   crate::arch::x86_64::registers::control::cr3::CR3::write(
                       root_table_frame,
                       address_space_id
                   );
               }

                _ => { unimplemented!() }
            }
        }
    }

    pub fn walk<E>(
        &self,
        _: impl FnMut(Option<(Depth, &Entry)>) -> ControlFlow<E>,
    ) -> ControlFlow<E> {
        todo!()

        //     #[allow(unreachable_code, unused_variables)]
        //     fn walk_impl<'a, E>(
        //         page_table: PageTable<SharedBorrow>,
        //         to_depth: Depth,
        //         func: &mut impl FnMut(Option<(Depth, &'a Entry)>) ->
        // ControlFlow<E>,     ) -> ControlFlow<E> {
        //         todo!(
        //             "I think this function is actually broken. It doesn't
        // traverse the address space correctly if huge pages are
        // enabled."         );

        //         page_table.iter().try_for_each(|entry| {
        //             let is_entry_intermediate = {
        //                 cfg_select! {
        //                     target_arch = "x86_64" => {
        //                         entry.is_huge() || current_depth ==
        // Depth::max()                     }

        //                     _ => { unimplemented!() }
        //                 }
        //             };

        //             if is_entry_intermediate {
        //                 func(Some((current_depth, entry)))
        //             } else if let Some(next_page_table) =
        // page_table.sub_table(entry_) {
        // walk_impl(next_page_table, current_depth.next(), to_depth,
        // func)             } else {                 let (steps, _) =
        // core::iter::Step::steps_between(&current_depth, &to_depth);
        // let iterations =
        // table_index_size().pow(u32::try_from(steps).unwrap());
        //                 (0..iterations).try_for_each(|_| func(None))
        //             }
        //         })
        //     }

        //     walk_impl(self.page_table(), Depth::min(), Depth::max(), &mut
        // func)
    }
}

impl core::fmt::Debug for Mapper {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Mapper").field(&self.root_table()).finish()
    }
}
