use crate::{
    mem::{
        HigherHalfDirectMap, Permissions,
        paging::{
            CreateEntryError, RootTable, WithEntryError,
            page_table::{Depth, Entry},
        },
        pmm::{FreeFrameError, LockFrameError, NextFrameError, PhysicalMemoryManager},
    },
    task::asid::AddressSpaceId,
};
use libsys::address::{Address, Frame, Page};

#[derive(Debug, Error)]
pub enum MappingError {
    #[error("attempted mapping to location outside memory")]
    OutsideMemory,

    #[error("attempted to lock a used frame")]
    CannotLock,

    #[error("attempted to map an already mapped region")]
    AlreadyMapped,

    #[error("ran out of memory for allocation")]
    OutOfMemory,
}

impl From<LockFrameError> for MappingError {
    fn from(error: LockFrameError) -> Self {
        match error {
            LockFrameError::OutOfBounds => Self::OutsideMemory,
            LockFrameError::NotLocked => Self::CannotLock,
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

impl From<FreeFrameError> for UnmappingError {
    fn from(error: FreeFrameError) -> Self {
        match error {
            FreeFrameError::OutOfBounds | FreeFrameError::NotLocked => Self::NotMapped,
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
            MappingError::OutsideMemory
            | MappingError::CannotLock
            | MappingError::AlreadyMapped => unreachable!(),
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

#[derive(Debug, Clone)]
pub struct Mapper(RootTable);

impl Mapper {
    pub fn new() -> Self {
        Self(RootTable::default())
    }

    pub fn root_table(&self) -> &RootTable {
        &self.0
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the frame.
    pub fn map(
        &mut self,
        page: Address<Page>,
        depth: Depth,
        frame: Address<Frame>,
        lock_frame: bool,
        memory_access: Permissions,
    ) -> Result<(), MappingError> {
        trace!(
            "Mapping: {page:X?} -> {frame:X?} {{ {memory_access:?}, {depth:?}, lock: {lock_frame} }}",
        );

        if lock_frame {
            PhysicalMemoryManager::lock_frame(frame)?;
        }

        // If acquisition of the frame is successful, attempt to map the page to the
        // frame index.
        self.0.with_entry_create(page, depth, |entry| {
            #[cfg(target_arch = "x86_64")]
            if depth > Depth::max() {
                entry.set_huge(true);
            }

            if HigherHalfDirectMap::is_address_higher_half(page.get()) {
                entry.set_global(true);
            } else {
                entry.set_user(true);
            }

            // Safety: Caller is required to maintain invariants.
            unsafe {
                entry.set_frame(frame);
            }

            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::instructions::__invlpg(page);
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
        self.0.with_entry_mut(page, to_depth, |entry| {
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

    pub fn auto_map(
        &mut self,
        page: Address<Page>,
        permissions: Permissions,
    ) -> Result<(), AutoMappingError> {
        let frame = PhysicalMemoryManager::next_frame(true)?;

        self.map(page, Depth::max(), frame, false, permissions)?;

        Ok(())
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: Address<Page>, depth: Option<Depth>) -> bool {
        self.0.with_entry(page, depth, |_| ()).is_ok()
    }

    pub fn is_mapped_to(&self, page: Address<Page>, frame: Address<Frame>) -> bool {
        self.0
            .with_entry(page, None, |entry| {
                entry
                    .get_frame()
                    .is_some_and(|entry_frame| entry_frame == frame)
            })
            .unwrap_or(false)
    }

    pub fn get_mapped_to(&self, page: Address<Page>) -> Result<Address<Frame>, GetMappingError> {
        self.0
            .with_entry(page, None, |entry| {
                entry.get_frame().ok_or(GetMappingError::NotMapped)
            })
            .map_err(|error| match error {
                super::paging::WithEntryError::NotMapped => GetMappingError::NotMapped,
                super::paging::WithEntryError::TerminatingPage => unreachable!(),
            })
            .flatten()
    }

    /* STATE CHANGING */

    pub fn get_permissions(&self, page: Address<Page>) -> Result<Permissions, GetMappingError> {
        let permissions = self.0.with_entry(page, None, Entry::get_permissions)?;

        Ok(permissions)
    }

    pub fn set_page_permissions(
        &mut self,
        page: Address<Page>,
        depth: Option<Depth>,
        permissions: Permissions,
    ) -> Result<(), GetMappingError> {
        self.0.with_entry_mut(page, depth, |entry| {
            entry.set_permissions(permissions);

            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::instructions::__invlpg(page);
        })?;

        Ok(())
    }

    pub unsafe fn swap_into(&self, address_space_id: AddressSpaceId) {
        let root_table_frame = self.root_table().frame();

        trace!("Swapping: {{ id: {address_space_id:?}, frame: {root_table_frame:X?} }}");

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            cfg_select! {
               target_arch = "x86_64" => {
                   crate::arch::x86_64::registers::control::cr3::CR3::write(
                       root_table_frame,
                       address_space_id
                   );
               }

                _ => { todo!() }
            }
        }
    }
}
