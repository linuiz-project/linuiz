use crate::{
    mem::{
        AddressSpaceId, HigherHalfDirectMap, Permissions,
        addr::{
            phys::{FrameAddress, StandardFrame},
            virt::{PageAddress, StandardPage, VirtualAddress},
        },
        clear_frame_memory,
        mapper::paging::{Depth, Entry},
        pmm::{FrameError, LockFrameError, PhysicalMemoryManager},
    },
    util::{ExclusiveBorrow, SharedBorrow},
};

pub mod paging;
use paging::{CreateEntryError, PageTable, WithEntryError};

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
pub struct Mapper(StandardFrame);

impl Mapper {
    pub fn new() -> Self {
        let frame = PhysicalMemoryManager::next_free_frame::<StandardFrame>()
            .inspect(|frame| {
                // Safety: Memory was just allocated, and is not otherwise aliased.
                unsafe {
                    clear_frame_memory(*frame);
                }
            })
            .expect("failed to allocate frame for new root page table");

        Self(frame)
    }

    pub fn frame(&self) -> StandardFrame {
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
    pub unsafe fn map<F: FrameAddress, P: PageAddress<Frame = F>>(
        &mut self,
        frame: F,
        page: P,
        lock_frame: bool,
        permissions: Permissions,
        invalidate_page: bool,
    ) -> Result<(), MappingError> {
        let depth = F::paging_depth();
        let address = VirtualAddress::from(page);

        trace!("Mapping ({permissions:?}): {page:X?} -> {frame:X?} {{ Lock: {lock_frame} }}",);

        if lock_frame {
            match PhysicalMemoryManager::lock_frame(frame) {
                Ok(()) | Err(LockFrameError::NotAllFree) => {}
                Err(error) => {
                    panic!("failed to lock frame for mapping: {error:?}");
                }
            }
        }

        // If acquisition of the frame is successful, attempt to map the page to the
        // frame index.
        self.root_table_mut()
            .with_entry_create(address, depth, |entry| {
                #[cfg(target_arch = "x86_64")]
                if depth > Depth::max() {
                    entry.set_huge(true);
                }

                if HigherHalfDirectMap::is_address_higher_half(address) {
                    entry.set_global(true);
                } else {
                    entry.set_user(true);
                }

                // Safety: Caller is required to maintain safety invariants.
                unsafe {
                    entry.set_address(frame);
                }

                // Safety: Caller is required to maintain safety invariants.
                unsafe {
                    entry.set_permissions(permissions);
                }

                entry.set_enabled();

                if invalidate_page {
                    cfg_select! {
                        target_arch = "x86_64" => {
                            crate::arch::x86_64::instructions::__invlpg(address);
                        }

                        _ => { unimplemented!() }
                    }
                }

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
    pub unsafe fn unmap<F: FrameAddress, P: PageAddress<Frame = F>>(
        &mut self,
        page: P,
        free_frame: bool,
    ) -> Result<(), UnmappingError> {
        let address = VirtualAddress::from(page);
        self.root_table_mut()
            .with_entry_mut(address, Some(P::paging_depth()), |entry| {
                let frame = entry.get_address().ok_or(UnmappingError::NotMapped)?;
                let frame = F::try_from(frame).unwrap();

                // Safety: Caller is required to maintain invariants.
                unsafe {
                    entry.clear();
                }

                if free_frame {
                    // Safety: Memory was just allocated.
                    unsafe {
                        PhysicalMemoryManager::free_frame(frame).unwrap();
                    }
                }

                // Invalidate the page in the TLB.
                #[cfg(target_arch = "x86_64")]
                crate::arch::x86_64::instructions::__invlpg(address);

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
        page: StandardPage,
        permissions: Permissions,
        invalidate_page: bool,
    ) -> Result<(), AutoMappingError> {
        let frame = PhysicalMemoryManager::next_free_frame::<StandardFrame>()
            .inspect(|frame| {
                // Safety: Memory was just allocated, and is not otherwise aliased.
                unsafe {
                    clear_frame_memory(*frame);
                }
            })
            .ok_or(AutoMappingError::OutOfMemory)?;

        // Safety:
        // - Frame was just allocated, so not current in use.
        // - Depth is the maximum paging depth, which is always supported.
        // - Caller is required to ensure permissions are correct.
        unsafe {
            self.map(frame, page, false, permissions, invalidate_page)?;
        }

        Ok(())
    }

    pub fn is_mapped(&self, address: VirtualAddress) -> bool {
        self.root_table().with_entry(address, None, |_| ()).is_ok()
    }

    pub fn get_permissions(&self, address: VirtualAddress) -> Result<Permissions, GetMappingError> {
        let permissions = self
            .root_table()
            .with_entry(address, None, Entry::get_permissions)?;

        Ok(permissions)
    }

    pub unsafe fn set_page_permissions(
        &mut self,
        address: VirtualAddress,
        depth: Option<Depth>,
        permissions: Permissions,
    ) -> Result<(), GetMappingError> {
        self.root_table_mut()
            .with_entry_mut(address, depth, |entry| {
                // Safety: Caller is required to maintain safety invariants.
                unsafe {
                    entry.set_permissions(permissions);
                }

                #[cfg(target_arch = "x86_64")]
                crate::arch::x86_64::instructions::__invlpg(address);
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

    pub fn walk_all(&self, func: impl Fn(Depth, usize, &Entry) + Copy) {
        self.root_table().walk_all(func);
    }

    #[cfg(debug_assertions)]
    pub fn pretty_print_table(&self) {
        self.walk_all(|depth, index, entry| {
            #[allow(clippy::as_conversions)]
            let print_offset = (Depth::min().get() - depth.get()) as usize;
            trace!("{:|>print_offset$} {index}: {entry:X?}", "");
        });
    }
}

impl core::fmt::Debug for Mapper {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Mapper").field(&self.root_table()).finish()
    }
}
