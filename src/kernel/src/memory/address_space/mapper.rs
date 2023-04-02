use crate::memory::{
    paging,
    paging::{Error, Result},
    PageDepth, PMM,
};
use libsys::{
    mem::{Mut, Ref},
    Address, Frame, Page,
};

pub struct Mapper {
    depth: PageDepth,
    root_frame: Address<Frame>,
    entry: paging::TableEntry,
}

// Safety: Type has no thread-local references.
unsafe impl Send for Mapper {}

impl Mapper {
    /// Attempts to construct a new page manager. Returns `None` if the PMM could not provide a root frame.
    pub fn new(depth: PageDepth) -> Option<Self> {
        PMM.next_frame().ok().map(|root_frame| {
            // Safety: Pointer is guaranteed valid due HHDM guarantee from kernel, and renting guarantees from PMM.
            unsafe {
                core::ptr::write_bytes(
                    crate::memory::hhdm_address().as_ptr().add(root_frame.get().get()),
                    0,
                    libsys::page_size().get(),
                )
            };

            Self { depth, root_frame, entry: paging::TableEntry::new(root_frame, paging::Attributes::PRESENT) }
        })
    }

    /// Safety
    ///
    /// - The root frame must point to a valid top-level page table.
    /// - There must only exist one copy of provided page table tree at any time.
    pub unsafe fn new_unsafe(depth: PageDepth, root_frame: Address<Frame>) -> Self {
        Self { depth, root_frame, entry: paging::TableEntry::new(root_frame, paging::Attributes::PRESENT) }
    }

    fn root_table(&self) -> paging::TableEntryCell<Ref> {
        // Safety: `Self` requires that the entry be valid.
        unsafe { paging::TableEntryCell::<Ref>::new(self.depth, &self.entry) }
    }

    fn root_table_mut(&mut self) -> paging::TableEntryCell<Mut> {
        // Safety: `Self` requires that the entry be valid.
        unsafe { paging::TableEntryCell::<Mut>::new(self.depth, &mut self.entry) }
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the specified frame index.
    pub fn map(
        &mut self,
        page: Address<Page>,
        to_depth: PageDepth,
        frame: Address<Frame>,
        lock_frame: bool,
        attributes: paging::Attributes,
    ) -> Result<()> {
        if lock_frame {
            // If the acquisition of the frame fails, return an error.
            PMM.lock_frame(frame).map_err(|_| Error::AllocError)?;
        }

        // If acquisition of the frame is successful, attempt to map the page to the frame index.
        let result = self
            .root_table_mut()
            // Safety: Frame does not contain any data.
            .with_entry_create(page, to_depth, |entry| unsafe {
                if to_depth > PageDepth::min() {
                    debug_assert!(
                        attributes.contains(paging::Attributes::HUGE),
                        "attributes missing huge bit for huge mapping"
                    );
                }

                entry.set(frame, attributes);

                #[cfg(target_arch = "x86_64")]
                crate::arch::x64::instructions::tlb::invlpg(page);
            });

        #[cfg(debug_assertions)]
        if result.is_ok() {
            debug_assert_eq!(self.get_mapped_to(page), Some(frame));
            debug_assert_eq!(self.get_page_attributes(page), Some(attributes));
        }

        result
    }

    /// Unmaps the given page, optionally freeing the frame the page points to within the given [`FrameManager`].
    ///
    /// Safety
    ///
    /// Caller must ensure calling this function does not cause memory corruption.
    pub unsafe fn unmap(&mut self, page: Address<Page>, to_depth: Option<PageDepth>, free_frame: bool) -> Result<()> {
        self.root_table_mut().with_entry_mut(page, to_depth, |entry| {
            // Safety: We've got an explicit directive from the caller to unmap this page, so the caller must ensure that's a valid operation.
            unsafe { entry.set_attributes(paging::Attributes::PRESENT, paging::AttributeModify::Remove) };

            let frame = entry.get_frame();
            // Safety: See above.
            unsafe { entry.set_frame(Address::new_truncate(0)) };

            if free_frame {
                PMM.free_frame(frame).unwrap();
            }

            // Invalidate the page in the TLB.
            #[cfg(target_arch = "x86_64")]
            crate::arch::x64::instructions::tlb::invlpg(page);
        })
    }

    pub fn auto_map(&mut self, page: Address<Page>, attributes: paging::Attributes) -> Result<()> {
        match PMM.next_frame() {
            Ok(frame) => {
                self.map(page, PageDepth::min(), frame, !attributes.contains(paging::Attributes::DEMAND), attributes)
            }
            Err(_) => Err(Error::AllocError),
        }
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: Address<Page>, depth: Option<PageDepth>) -> bool {
        self.root_table().with_entry(page, depth, |_| ()).is_ok()
    }

    pub fn is_mapped_to(&self, page: Address<Page>, frame: Address<Frame>) -> bool {
        self.root_table().with_entry(page, None, |entry| entry.get_frame() == frame).unwrap_or(false)
    }

    pub fn get_mapped_to(&self, page: Address<Page>) -> Option<Address<Frame>> {
        self.root_table().with_entry(page, None, |entry| entry.get_frame()).ok()
    }

    /* STATE CHANGING */

    pub fn get_page_attributes(&self, page: Address<Page>) -> Option<paging::Attributes> {
        self.root_table().with_entry(page, None, |entry| entry.get_attributes()).ok()
    }

    pub unsafe fn set_page_attributes(
        &mut self,
        page: Address<Page>,
        depth: Option<PageDepth>,
        attributes: paging::Attributes,
        modify_mode: paging::AttributeModify,
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
        #[cfg(target_arch = "x86_64")]
        crate::arch::x64::registers::control::CR3::write(
            self.root_frame,
            crate::arch::x64::registers::control::CR3Flags::empty(),
        );
    }

    pub fn view_root_page_table<'a>(&'a self) -> &'a [paging::TableEntry; const { libsys::table_index_size().get() }] {
        // Safety: Root frame is guaranteed to be valid within the HHDM.
        let table_ptr = unsafe { crate::memory::hhdm_address().as_ptr().add(self.root_frame.get().get()).cast() };
        // Safety: Root frame is guaranteed to be valid for PTEs for the length of the table index size.
        let table = unsafe { core::slice::from_raw_parts(table_ptr, libsys::table_index_size().get()) };
        // Safety: Table was created to match the size required by return type.
        unsafe { table.try_into().unwrap_unchecked() }
    }
}
