use crate::{
    addr_ty::{Physical, Virtual},
    memory::{
        falloc,
        paging::{AttributeModify, Level4, PageAttributes, PageTable, PageTableEntry},
        Page,
    },
    Address,
};

#[derive(Debug, Clone, Copy)]
pub enum MapError {
    NotMapped,
    AlreadyMapped,
    FallocError(crate::memory::falloc::FallocError),
}

pub struct VirtualAddressor {
    mapped_page: Page,
    pml4_index: usize,
}

impl VirtualAddressor {
    pub const fn null() -> Self {
        Self {
            mapped_page: Page::null(),
            pml4_index: usize::MAX,
        }
    }

    /// Attempts to create a new VirtualAddressor, with `mapped_page` specifying the current page
    /// where the entirety of the system physical memory is mapped.
    ///
    /// Safety: this method is unsafe because `mapped_page` can be any value; that is, not necessarily
    /// a valid address in which physical memory is already mapped. The expectation is that `mapped_page` is
    /// a propery starting page for the physical memory mapping.
    pub unsafe fn new(mapped_page: Page) -> Self {
        let pml4_index = falloc::get()
            .lock_next()
            .expect("Failed to lock frame for virtual addressor's PML4");

        // Clear PML4 frame.
        core::ptr::write_bytes(
            mapped_page.forward(pml4_index).unwrap().as_mut_ptr::<u8>(),
            0,
            0x1000,
        );

        Self {
            // We don't know where physical memory is mapped at this point,
            // so rely on what the caller specifies for us.
            mapped_page,
            pml4_index,
        }
    }

    pub fn mapped_offset(&self) -> Address<Virtual> {
        self.mapped_page.base_addr()
    }

    /* ACQUIRE STATE */

    pub fn pml4_addr(&self) -> Address<Physical> {
        Address::<Physical>::new(self.pml4_index * 0x1000)
    }

    fn pml4_page(&self) -> Page {
        self.mapped_page.forward(self.pml4_index).unwrap()
    }

    fn pml4(&self) -> Option<&PageTable<Level4>> {
        unsafe { self.pml4_page().as_ptr::<PageTable<Level4>>().as_ref() }
    }

    fn pml4_mut(&mut self) -> Option<&mut PageTable<Level4>> {
        unsafe { self.pml4_page().as_mut_ptr::<PageTable<Level4>>().as_mut() }
    }

    fn get_page_entry(&self, page: &Page) -> Option<&PageTableEntry> {
        let mapped_page = self.mapped_page;
        let addr = page.base_addr();

        unsafe {
            self.pml4()
                .and_then(|p4| p4.sub_table(addr.p4_index(), mapped_page))
                .and_then(|p3| p3.sub_table(addr.p3_index(), mapped_page))
                .and_then(|p2| p2.sub_table(addr.p2_index(), mapped_page))
                .and_then(|p1| Some(p1.get_entry(addr.p1_index())))
        }
    }

    fn get_page_entry_mut(&mut self, page: &Page) -> Option<&mut PageTableEntry> {
        let mapped_page = self.mapped_page;
        let addr = page.base_addr();

        unsafe {
            self.pml4_mut()
                .and_then(|p4| p4.sub_table_mut(addr.p4_index(), mapped_page))
                .and_then(|p3| p3.sub_table_mut(addr.p3_index(), mapped_page))
                .and_then(|p2| p2.sub_table_mut(addr.p2_index(), mapped_page))
                .and_then(|p1| Some(p1.get_entry_mut(addr.p1_index())))
        }
    }

    fn get_page_entry_create(&mut self, page: &Page) -> &mut PageTableEntry {
        let mapped_page = self.mapped_page;
        let addr = page.base_addr();

        unsafe {
            self.pml4_mut()
                .unwrap()
                .sub_table_create(addr.p4_index(), mapped_page)
                .sub_table_create(addr.p3_index(), mapped_page)
                .sub_table_create(addr.p2_index(), mapped_page)
                .get_entry_mut(addr.p1_index())
        }
    }

    /* MAP / UNMAP */

    /// Maps the specified frame to the specified frame index.
    ///
    /// The `req_falloc` parameter is set as follows:
    ///     `Some` or `None` indicates whether frame allocation is required,
    ///         `true` or `false` in a `Some` indicates whether the allocation is locked.
    pub fn map(
        &mut self,
        page: &Page,
        frame_index: usize,
        req_falloc: Option<bool>,
    ) -> Result<(), MapError> {
        // Attempt to acquire the requisite frame, following the outlined parsing of `req_falloc`.
        match req_falloc {
            Some(true) => falloc::get().lock(frame_index),
            Some(false) => falloc::get().borrow(frame_index),
            None => Ok(frame_index),
        }
        // If the acquisition of the frame fails, transform the error.
        .map_err(|falloc_err| MapError::FallocError(falloc_err))
        // If acquisition of the frame is successful, map the page to the frame index.
        .map(|frame_index| {
            self.get_page_entry_create(page).set(
                frame_index,
                PageAttributes::PRESENT | PageAttributes::WRITABLE,
            );

            crate::instructions::tlb::invalidate(page);
        })
    }

    pub fn unmap(&mut self, page: &Page, locked: bool) -> Result<(), MapError> {
        self.get_page_entry_mut(page)
            .map(|entry| {
                entry.set_attributes(PageAttributes::PRESENT, AttributeModify::Remove);

                // Drop pointed frame since the entry is no longer present.
                unsafe {
                    if locked {
                        falloc::get().free(entry.take_frame_index()).unwrap();
                    } else {
                        falloc::get().drop(entry.take_frame_index()).unwrap();
                    }
                }

                // Invalidate the page in the TLB.
                crate::instructions::tlb::invalidate(page);
            })
            .ok_or(MapError::NotMapped)
    }

    pub fn copy_by_map(&mut self, unmap_from: &Page, map_to: &Page) -> Result<(), MapError> {
        // TODO possibly elide this check for perf?
        if self.is_mapped(map_to.base_addr()) {
            return Err(MapError::AlreadyMapped);
        }

        match self.get_page_entry_mut(unmap_from).map(|entry| {
            entry.set_attributes(PageAttributes::PRESENT, AttributeModify::Remove);
            unsafe { entry.take_frame_index() }
        }) {
            Some(frame_index) => {
                self.map(map_to, frame_index, None).unwrap();
                crate::instructions::tlb::invalidate(unmap_from);

                Ok(())
            }
            None => Err(MapError::NotMapped),
        }
    }

    pub fn automap(&mut self, page: &Page) {
        self.map(page, falloc::get().lock_next().unwrap(), None)
            .unwrap();
    }

    /// Attempts to create a 1:1 mapping between a virual memory page and its physical counterpart.
    ///
    /// REMARK:
    ///     This function assumes the frame for the identity mapping is:
    ///         A) a valid lock or borrow
    ///         B) not required for the mapping
    pub fn identity_map(&mut self, page: &Page) -> Result<(), MapError> {
        self.map(page, page.index(), None)
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, virt_addr: Address<Virtual>) -> bool {
        self.get_page_entry(&Page::containing_addr(virt_addr))
            .map_or(false, |entry| {
                entry.get_attributes().contains(PageAttributes::PRESENT)
            })
    }

    pub fn is_mapped_to(&self, page: &Page, frame_index: usize) -> bool {
        match self
            .get_page_entry(page)
            .and_then(|entry| entry.get_frame_index())
        {
            Some(entry_frame_index) => frame_index == entry_frame_index,
            None => false,
        }
    }

    pub fn translate_page(&self, page: &Page) -> Option<usize> {
        self.get_page_entry(page)
            .and_then(|entry| entry.get_frame_index())
    }

    /* STATE CHANGING */

    pub unsafe fn modify_mapped_page(&mut self, page: Page) {
        for index in 0..falloc::get().total_frame_count() {
            self.get_page_entry_create(&page.forward(index).unwrap())
                .set(index, PageAttributes::PRESENT | PageAttributes::WRITABLE);

            crate::instructions::tlb::invalidate(&page);
        }

        self.mapped_page = page;
    }

    pub fn get_page_attribs(&self, page: &Page) -> Option<PageAttributes> {
        self.get_page_entry(page)
            .map(|page_entry| page_entry.get_attributes())
    }

    pub unsafe fn set_page_attribs(
        &mut self,
        page: &Page,
        attributes: PageAttributes,
        modify_mode: AttributeModify,
    ) {
        self.get_page_entry_mut(page)
            .map(|page_entry| page_entry.set_attributes(attributes, modify_mode));
    }

    pub unsafe fn swap_into(&self) {
        let addr = self.pml4_addr();
        trace!("Swapping {:?} into CR3.", addr);
        crate::registers::CR3::write(addr, crate::registers::CR3Flags::empty());
    }

    /// Returns `true` if the current CR3 address matches the addressor's
    /// PML4 frame, and `false` otherwise.
    pub fn is_swapped_in(&self) -> bool {
        crate::registers::CR3::read().0.frame_index() == self.pml4_index
    }
}
