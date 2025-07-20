use crate::mem::{
    HigherHalfDirectMap, Permissions,
    paging::{
        RootTable,
        page_table::{Depth, Entry},
    },
    pmm::PhysicalMemoryManager,
};
use libsys::{Address, Frame, Page};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("could not allocate memory for mapping")]
    OutOfMemory,

    #[error(transparent)]
    Paging(#[from] crate::mem::paging::Error),
}

#[derive(Debug, FromZeros, Clone)]
pub struct Mapper(RootTable);

// Safety: Type has no thread-local references.
unsafe impl Send for Mapper {}

impl Mapper {
    pub fn new() -> Self {
        zerocopy::FromZeros::new_zeroed()
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the frame.
    pub fn map(
        &mut self,
        page: Address<Page>,
        depth: Depth,
        frame: Address<Frame>,
        lock_frame: bool,
        permissions: Permissions,
    ) -> Result<(), Error> {
        trace!(
            "Mapping: {page:X?} -> {frame:X?}  (to_depth:{}, lock:{lock_frame} {permissions:?})",
            depth.get()
        );

        if lock_frame {
            PhysicalMemoryManager::lock_frame(frame);
        }

        // If acquisition of the frame is successful, attempt to map the page to the frame index.
        self.0.with_entry_create(page, depth, |entry| {
            #[cfg(target_arch = "x86_64")]
            if depth > Depth::max() {
                unsafe {
                    entry.set_huge(true);
                }
            }

            #[cfg(target_arch = "riscv64")]
            if HigherHalfDirectMap::is_address_higher_half(page) {
                unsafe {
                    entry.set_global(true);
                }
            }

            unsafe {
                entry.set_frame(frame);
            }

            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::instructions::__invlpg(page);
        })?;

        Ok(())
    }

    /// Unmaps the given page, optionally freeing the frame the page points to within the given [`FrameManager`].
    ///
    /// # Safety
    ///
    /// Caller must ensure calling this function does not cause memory corruption.
    pub unsafe fn unmap(
        &mut self,
        page: Address<Page>,
        to_depth: Option<Depth>,
        free_frame: bool,
    ) -> Result<(), Error> {
        self.0.with_entry_mut(page, to_depth, |entry| {
            let frame = entry.get_frame();

            unsafe {
                entry.clear();
            }

            if free_frame {
                PhysicalMemoryManager::free_frame(frame);
            }

            // Invalidate the page in the TLB.
            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::instructions::__invlpg(page);
        })?;

        Ok(())
    }

    pub fn auto_map(&mut self, page: Address<Page>, permissions: Permissions) -> Result<(), Error> {
        let frame = PhysicalMemoryManager::next_frame().ok_or(Error::OutOfMemory)?;

        self.map(page, Depth::max(), frame, false, permissions)?;

        Ok(())
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: Address<Page>, depth: Option<Depth>) -> bool {
        self.0.with_entry(page, depth, |_| ()).is_ok()
    }

    pub fn is_mapped_to(&self, page: Address<Page>, frame: Address<Frame>) -> bool {
        self.0
            .with_entry(page, None, |entry| entry.get_frame() == frame)
            .unwrap_or(false)
    }

    pub fn get_mapped_to(&self, page: Address<Page>) -> Option<Address<Frame>> {
        self.0
            .with_entry(page, None, |entry| entry.get_frame())
            .ok()
    }

    /* STATE CHANGING */

    pub fn get_permissions(&self, page: Address<Page>) -> Result<Permissions, Error> {
        let permissions = self.0.with_entry(page, None, Entry::get_permissions)?;

        Ok(permissions)
    }

    pub unsafe fn set_page_permissions(
        &mut self,
        page: Address<Page>,
        depth: Option<Depth>,
        permissions: Permissions,
    ) -> Result<(), Error> {
        self.0.with_entry_mut(page, depth, |entry| {
            // Safety: Caller is required to maintain safety invariants.
            unsafe {
                entry.set_permissions(permissions);
            }

            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::instructions::__invlpg(page);
        })?;

        Ok(())
    }

    pub fn get_root_table_address(&self) -> Address<Frame> {
        let self_ptr = core::ptr::from_ref(self).cast_mut();
        let self_page = Address::<Page>::from_ptr(self_ptr);

        HigherHalfDirectMap::page_to_frame(self_page)
    }

    pub unsafe fn swap_into(&self) {
        let root_table_address = self.get_root_table_address();

        trace!("Swapping: {root_table_address:X?}");

        cfg_select! {
            target_arch = "x86_64" => {
                // Safety: Caller is required to maintain safety invariants.
                unsafe {
                    crate::arch::x86_64::registers::control::CR3::write(
                        root_table_address,
                        crate::arch::x86_64::registers::control::CR3Flags::empty(),
                    );
                }
            }
        }
    }
}
