use crate::{
    mem::{
        HigherHalfDirectMap,
        paging::{Error, FlagsModify, PageTable, PageTableEntry, TableDepth, TableEntryFlags},
        pmm::PhysicalMemoryManager,
    },
    util::{Mut, Ref},
};
use libsys::{Address, Frame, Page};

pub struct Mapper {
    depth: TableDepth,
    root_frame: Address<Frame>,
    entry: PageTableEntry,
}

// Safety: Type has no thread-local references.
unsafe impl Send for Mapper {}

impl Mapper {
    /// Attempts to construct a new page manager. Returns `None` if the `pmm::get()` could not provide a root frame.
    pub fn new(depth: TableDepth) -> Self {
        let root_frame = PhysicalMemoryManager::next_frame()
            .expect("could not retrieve a frame for mapper creation");

        // Safety: `root_frame` is a physical address to a page-sized allocation, which is then offset to the HHDM.
        unsafe {
            core::ptr::write_bytes(
                core::ptr::with_exposed_provenance_mut::<u8>(
                    HigherHalfDirectMap::frame_to_page(root_frame).get().get(),
                ),
                0u8,
                libsys::page_size(),
            );
        }

        Self {
            depth,
            root_frame,
            entry: PageTableEntry::new(root_frame, TableEntryFlags::PRESENT),
        }
    }

    /// # Safety
    ///
    /// - The root frame must point to a valid top-level page table.
    /// - There must only exist one copy of provided page table tree at any time.
    pub unsafe fn new_unsafe(depth: TableDepth, root_frame: Address<Frame>) -> Self {
        Self {
            depth,
            root_frame,
            entry: PageTableEntry::new(root_frame, TableEntryFlags::PRESENT),
        }
    }

    fn root_table(&self) -> PageTable<'_, Ref> {
        // Safety: `Self` requires that the entry be valid.
        unsafe { PageTable::<Ref>::new(self.depth, &self.entry) }
    }

    fn root_table_mut(&mut self) -> PageTable<'_, Mut> {
        // Safety: `Self` requires that the entry be valid.
        unsafe { PageTable::<Mut>::new(self.depth, &mut self.entry) }
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the frame.
    pub fn map(
        &mut self,
        page: Address<Page>,
        depth: TableDepth,
        frame: Address<Frame>,
        lock_frame: bool,
        attributes: TableEntryFlags,
    ) -> Result<(), Error> {
        trace!(
            "Mapping: {page:X?} -> {frame:X?}  (to_depth:{}, lock:{lock_frame}, {attributes:?})",
            depth.get()
        );

        if lock_frame {
            PhysicalMemoryManager::lock_frame(frame)?;
        }

        // If acquisition of the frame is successful, attempt to map the page to the frame index.
        self.root_table_mut()
            .with_entry_create(page, depth, |entry| {
                if depth > TableDepth::min() {
                    debug_assert!(
                        attributes.contains(TableEntryFlags::HUGE),
                        "attributes missing huge bit for huge mapping"
                    );
                }

                *entry = PageTableEntry::new(frame, attributes);

                #[cfg(target_arch = "x86_64")]
                crate::arch::x86_64::instructions::__invlpg(page);
            })
    }

    /// Unmaps the given page, optionally freeing the frame the page points to within the given [`FrameManager`].
    ///
    /// # Safety
    ///
    /// Caller must ensure calling this function does not cause memory corruption.
    pub unsafe fn unmap(
        &mut self,
        page: Address<Page>,
        to_depth: Option<TableDepth>,
        free_frame: bool,
    ) -> Result<(), Error> {
        self.root_table_mut()
            .with_entry_mut(page, to_depth, |entry| {
                // Safety: Caller must ensure invariants are maintained.
                unsafe {
                    entry.set_attributes(TableEntryFlags::PRESENT, FlagsModify::Remove);
                }

                let frame = entry.get_frame();

                // Safety: Caller must ensure invariants are maintained.
                unsafe {
                    entry.set_frame(Address::new_truncate(0));
                }

                if free_frame {
                    PhysicalMemoryManager::free_frame(frame)?;
                }

                // Invalidate the page in the TLB.
                #[cfg(target_arch = "x86_64")]
                crate::arch::x86_64::instructions::__invlpg(page);

                Ok(())
            })
            .flatten()
    }

    pub fn auto_map(&mut self, page: Address<Page>, flags: TableEntryFlags) -> Result<(), Error> {
        let frame = PhysicalMemoryManager::next_frame()?;

        self.map(page, TableDepth::min(), frame, false, flags)?;

        Ok(())
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: Address<Page>, depth: Option<TableDepth>) -> bool {
        self.root_table().with_entry(page, depth, |_| ()).is_ok()
    }

    pub fn is_mapped_to(&self, page: Address<Page>, frame: Address<Frame>) -> bool {
        self.root_table()
            .with_entry(page, None, |entry| entry.get_frame() == frame)
            .unwrap_or(false)
    }

    pub fn get_mapped_to(&self, page: Address<Page>) -> Option<Address<Frame>> {
        self.root_table()
            .with_entry(page, None, |entry| entry.get_frame())
            .ok()
    }

    /* STATE CHANGING */

    pub fn get_page_attributes(&self, page: Address<Page>) -> Option<TableEntryFlags> {
        self.root_table()
            .with_entry(page, None, |entry| entry.get_attributes())
            .ok()
    }

    /// # Safety
    ///
    /// TODO
    pub unsafe fn set_page_attributes(
        &mut self,
        page: Address<Page>,
        depth: Option<TableDepth>,
        attributes: TableEntryFlags,
        modify_mode: FlagsModify,
    ) -> Result<(), Error> {
        self.root_table_mut().with_entry_mut(page, depth, |entry| {
            // Safety: Caller is required to maintain safety invariants.
            unsafe {
                entry.set_attributes(attributes, modify_mode);
            }

            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::instructions::__invlpg(page);
        })
    }

    /// # Safety
    ///
    /// Caller must ensure that switching the currently active address space will not cause undefined behaviour.
    pub unsafe fn swap_into(&self) {
        trace!("Swapping CR3: {:X?}", self.root_frame);

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::registers::control::CR3::write(
                self.root_frame,
                crate::arch::x86_64::registers::control::CR3Flags::empty(),
            );
        }
    }

    pub fn root_frame(&self) -> Address<Frame> {
        self.root_frame
    }

    pub fn view_page_table(&self) -> &[PageTableEntry; libsys::table_index_size()] {
        // Safety: Root frame is guaranteed to be valid within the HHDM.
        let table_ptr = core::ptr::with_exposed_provenance(
            HigherHalfDirectMap::frame_to_page(self.root_frame)
                .get()
                .get(),
        );
        // Safety: Root frame is guaranteed to be valid for PTEs for the length of the table index size.
        let table = unsafe { core::slice::from_raw_parts(table_ptr, libsys::table_index_size()) };
        // Safety: Table was created to match the size required by return type.
        unsafe { table.try_into().unwrap_unchecked() }
    }
}
