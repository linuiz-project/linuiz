use crate::mem::{
    alloc::pmm,
    paging,
    paging::{Error, Result, TableDepth},
    HHDM,
};
use libkernel::mem::{Mut, Ref};
use libsys::{Address, Frame, Page};

pub struct Mapper {
    depth: TableDepth,
    root_frame: Address<Frame>,
    entry: paging::PageTableEntry,
}

// Safety: Type has no thread-local references.
unsafe impl Send for Mapper {}

impl Mapper {
    /// Attempts to construct a new page manager. Returns `None` if the pmm::get() could not provide a root frame.
    pub fn new(depth: TableDepth) -> Option<Self> {
        let root_frame = pmm::get().next_frame().ok()?;
        trace!("New mapper root frame: {:X}", root_frame);

        // Safety: pmm::get() promises rented frames to be within the HHDM.
        unsafe {
            let hhdm_offset_address = HHDM.offset(root_frame).unwrap();
            core::ptr::write_bytes(hhdm_offset_address.as_ptr(), 0x0, libsys::page_size());
        }

        Some(Self {
            depth,
            root_frame,
            entry: paging::PageTableEntry::new(root_frame, paging::TableEntryFlags::PRESENT),
        })
    }

    /// Safety
    ///
    /// - The root frame must point to a valid top-level page table.
    /// - There must only exist one copy of provided page table tree at any time.
    pub unsafe fn new_unsafe(depth: TableDepth, root_frame: Address<Frame>) -> Self {
        Self { depth, root_frame, entry: paging::PageTableEntry::new(root_frame, paging::TableEntryFlags::PRESENT) }
    }

    const fn root_table(&self) -> paging::PageTable<Ref> {
        // Safety: `Self` requires that the entry be valid.
        unsafe { paging::PageTable::<Ref>::new(self.depth, &self.entry) }
    }

    fn root_table_mut(&mut self) -> paging::PageTable<Mut> {
        // Safety: `Self` requires that the entry be valid.
        unsafe { paging::PageTable::<Mut>::new(self.depth, &mut self.entry) }
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the specified frame index.
    pub fn map(
        &mut self,
        page: Address<Page>,
        depth: TableDepth,
        frame: Address<Frame>,
        lock_frame: bool,
        attributes: paging::TableEntryFlags,
    ) -> Result<()> {
        if lock_frame {
            // If the acquisition of the frame fails, return an error.
            pmm::get().lock_frame(frame).map_err(|err| match err {
                super::alloc::pmm::Error::OutOfBounds => Error::FrameBounds,
                _ => Error::AllocError,
            })?;
        }

        // If acquisition of the frame is successful, attempt to map the page to the frame index.
        let result = self
            .root_table_mut()
            // Safety: Frame does not contain any data.
            .with_entry_create(page, depth, |entry| {
                if depth > TableDepth::min() {
                    debug_assert!(
                        attributes.contains(paging::TableEntryFlags::HUGE),
                        "attributes missing huge bit for huge mapping"
                    );
                }

                *entry = paging::PageTableEntry::new(frame, attributes);

                #[cfg(target_arch = "x86_64")]
                crate::arch::x64::instructions::tlb::invlpg(page);
            });

        result
    }

    /// Unmaps the given page, optionally freeing the frame the page points to within the given [`FrameManager`].
    ///
    /// Safety
    ///
    /// Caller must ensure calling this function does not cause memory corruption.
    pub unsafe fn unmap(&mut self, page: Address<Page>, to_depth: Option<TableDepth>, free_frame: bool) -> Result<()> {
        self.root_table_mut().with_entry_mut(page, to_depth, |entry| {
            // Safety: We've got an explicit directive from the caller to unmap this page, so the caller must ensure that's a valid operation.
            unsafe { entry.set_attributes(paging::TableEntryFlags::PRESENT, paging::FlagsModify::Remove) };

            let frame = entry.get_frame();
            // Safety: See above.
            unsafe { entry.set_frame(Address::new_truncate(0)) };

            if free_frame {
                pmm::get().free_frame(frame).unwrap();
            }

            // Invalidate the page in the TLB.
            #[cfg(target_arch = "x86_64")]
            crate::arch::x64::instructions::tlb::invlpg(page);
        })
    }

    pub fn auto_map(&mut self, page: Address<Page>, flags: paging::TableEntryFlags) -> Result<()> {
        match pmm::get().next_frame() {
            Ok(frame) => self.map(page, TableDepth::min(), frame, false, flags),
            Err(err) => {
                trace!("Auto alloc pmm::get() error: {:?}", err);
                Err(Error::AllocError)
            }
        }
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: Address<Page>, depth: Option<TableDepth>) -> bool {
        self.root_table().with_entry(page, depth, |_| ()).is_ok()
    }

    pub fn is_mapped_to(&self, page: Address<Page>, frame: Address<Frame>) -> bool {
        self.root_table().with_entry(page, None, |entry| entry.get_frame() == frame).unwrap_or(false)
    }

    pub fn get_mapped_to(&self, page: Address<Page>) -> Option<Address<Frame>> {
        self.root_table().with_entry(page, None, |entry| entry.get_frame()).ok()
    }

    /* STATE CHANGING */

    pub fn get_page_attributes(&self, page: Address<Page>) -> Option<paging::TableEntryFlags> {
        self.root_table().with_entry(page, None, |entry| entry.get_attributes()).ok()
    }

    pub unsafe fn set_page_attributes(
        &mut self,
        page: Address<Page>,
        depth: Option<TableDepth>,
        attributes: paging::TableEntryFlags,
        modify_mode: paging::FlagsModify,
    ) -> Result<()> {
        self.root_table_mut().with_entry_mut(page, depth, |entry| {
            entry.set_attributes(attributes, modify_mode);

            #[cfg(target_arch = "x86_64")]
            crate::arch::x64::instructions::tlb::invlpg(page);
        })
    }

    /// Safety
    ///
    /// Caller must ensure that switching the currently active address space will not cause undefined behaviour.
    pub unsafe fn swap_into(&self) {
        trace!("Swapping address space to: {:X}", self.root_frame);

        #[cfg(target_arch = "x86_64")]
        crate::arch::x64::registers::control::CR3::write(
            self.root_frame,
            crate::arch::x64::registers::control::CR3Flags::empty(),
        );
    }

    pub const fn root_frame(&self) -> Address<Frame> {
        self.root_frame
    }

    pub fn view_page_table(&self) -> &[paging::PageTableEntry; libsys::table_index_size()] {
        // Safety: Root frame is guaranteed to be valid within the HHDM.
        let table_ptr = HHDM.offset(self.root_frame).unwrap().as_ptr().cast();
        // Safety: Root frame is guaranteed to be valid for PTEs for the length of the table index size.
        let table = unsafe { core::slice::from_raw_parts(table_ptr, libsys::table_index_size()) };
        // Safety: Table was created to match the size required by return type.
        unsafe { table.try_into().unwrap_unchecked() }
    }
}
