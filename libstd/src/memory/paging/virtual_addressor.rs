use crate::{
    addr_ty::{Physical, Virtual},
    memory::{
        paging::{AttributeModify, Level4, PageAttributes, PageTable, PageTableEntry},
        Frame, Page,
    },
    Address,
};

pub struct VirtualAddressor {
    mapped_page: Page,
    pml4_frame: Frame,
}

impl VirtualAddressor {
    pub const fn null() -> Self {
        Self {
            mapped_page: Page::null(),
            pml4_frame: Frame::null(),
        }
    }

    /// Attempts to create a new VirtualAddressor, with `current_mapped_addr` specifying the current virtual
    /// address where the entirety of the system physical memory is mapped.
    ///
    /// Safety: this method is unsafe because `mapped_page` can be any value; that is, not necessarily
    /// a valid address in which physical memory is already mapped.
    pub unsafe fn new(mapped_page: Page) -> Self {
        let pml4_frame = crate::memory::falloc::get()
            .autolock()
            .expect("failed to lock frame for PML4 of VirtualAddressor");

        debug!(
            "Virtual addressor created:\n\tframe {:?}\tmapped {:?}",
            pml4_frame, mapped_page
        );

        // Clear PML4 frame.
        core::ptr::write_bytes(
            (mapped_page.base_addr() + pml4_frame.base_addr().as_usize()).as_mut_ptr::<u8>(),
            0,
            0x1000,
        );

        Self {
            // we don't know where physical memory is mapped at this point,
            // so rely on what the caller specifies for us
            mapped_page,
            pml4_frame,
        }
    }

    pub fn mapped_offset(&self) -> Address<Virtual> {
        self.mapped_page.base_addr()
    }

    /* ACQUIRE STATE */

    pub fn base_addr(&self) -> Address<Physical> {
        self.pml4_frame.base_addr()
    }

    pub fn pml4_addr(&self) -> Address<Physical> {
        self.pml4_frame.base_addr()
    }

    fn pml4_page(&self) -> Page {
        self.mapped_page.forward(self.pml4_frame.index()).unwrap()
    }

    fn pml4(&self) -> &PageTable<Level4> {
        unsafe { &*self.pml4_page().as_ptr() }
    }

    fn pml4_mut(&mut self) -> &mut PageTable<Level4> {
        unsafe { &mut *self.pml4_page().as_mut_ptr() }
    }

    fn get_page_entry(&self, page: &Page) -> Option<&PageTableEntry> {
        let offset = self.mapped_page.base_addr();
        let addr = page.base_addr();

        unsafe {
            self.pml4()
                .sub_table(addr.p4_index(), offset)
                .and_then(|p3| p3.sub_table(addr.p3_index(), offset))
                .and_then(|p2| p2.sub_table(addr.p2_index(), offset))
                .and_then(|p1| Some(p1.get_entry(addr.p1_index())))
        }
    }

    fn get_page_entry_mut(&mut self, page: &Page) -> Option<&mut PageTableEntry> {
        let offset = self.mapped_offset();
        let addr = page.base_addr();

        unsafe {
            self.pml4_mut()
                .sub_table_mut(addr.p4_index(), offset)
                .and_then(|p3| p3.sub_table_mut(addr.p3_index(), offset))
                .and_then(|p2| p2.sub_table_mut(addr.p2_index(), offset))
                .and_then(|p1| Some(p1.get_entry_mut(addr.p1_index())))
        }
    }

    fn get_page_entry_create(&mut self, page: &Page) -> &mut PageTableEntry {
        let offset = self.mapped_page.base_addr();
        let addr = page.base_addr();

        unsafe {
            self.pml4_mut()
                .sub_table_create(addr.p4_index(), offset)
                .sub_table_create(addr.p3_index(), offset)
                .sub_table_create(addr.p2_index(), offset)
                .get_entry_mut(addr.p1_index())
        }
    }

    /* MAP / UNMAP */

    pub fn map(&mut self, page: &Page, frame: &Frame) {
        assert!(
            !self.is_mapped(page.base_addr()),
            "Page is already mapped: {:?}",
            page
        );

        self.get_page_entry_create(page)
            .set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
        crate::instructions::tlb::invalidate(page);
        trace!("Mapped {:?} -> {:?}", page, frame);

        assert!(self.is_mapped_to(page, frame), "failed to map page",);
    }

    pub fn unmap(&mut self, page: &Page) {
        assert!(self.is_mapped(page.base_addr()), "page already unmapped");

        self.get_page_entry_mut(page)
            .unwrap()
            .set_attributes(PageAttributes::PRESENT, AttributeModify::Remove);
        crate::instructions::tlb::invalidate(page);

        assert!(!self.is_mapped(page.base_addr()), "failed to unmap page",);
    }

    pub fn identity_map(&mut self, frame: &Frame) {
        self.map(&Page::from_index(frame.index()), frame);
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, virt_addr: Address<Virtual>) -> bool {
        self.get_page_entry(&Page::containing_addr(virt_addr))
            .map_or(false, |entry| {
                entry.get_attributes().contains(PageAttributes::PRESENT)
            })
    }

    pub fn is_mapped_to(&self, page: &Page, frame: &Frame) -> bool {
        match self
            .get_page_entry(page)
            .and_then(|entry| entry.get_frame())
        {
            Some(entry_frame) => frame.index() == entry_frame.index(),
            None => false,
        }
    }

    pub fn translate_page(&self, page: &Page) -> Option<Frame> {
        self.get_page_entry(page)
            .and_then(|entry| entry.get_frame())
    }

    /* STATE CHANGING */

    pub unsafe fn modify_mapped_page(&mut self, page: Page) {
        let total_memory_pages = crate::memory::falloc::get().total_memory(None) / 0x1000;
        for index in 0..total_memory_pages {
            self.map(&page.forward(index).unwrap(), &Frame::from_index(index));
        }

        self.mapped_page = page;
    }

    pub unsafe fn get_page_attributes(&self, page: &Page) -> Option<PageAttributes> {
        self.get_page_entry(page)
            .map(|page_entry| page_entry.get_attributes())
    }

    pub unsafe fn set_page_attributes(
        &mut self,
        page: &Page,
        attributes: PageAttributes,
        modify_mode: AttributeModify,
    ) {
        self.get_page_entry_mut(page)
            .map(|page_entry| page_entry.set_attributes(attributes, modify_mode));
    }

    pub unsafe fn swap_into(&self) {
        trace!("Swapping {:?} into CR3.", self.pml4_frame.base_addr());
        crate::registers::CR3::write(&self.pml4_frame, crate::registers::CR3Flags::empty());
    }

    /* MISC */

    fn validate_entry(
        index4: Option<usize>,
        index3: Option<usize>,
        index2: Option<usize>,
        index1: Option<usize>,
        entry: &PageTableEntry,
    ) {
        match entry.validate() {
            Ok(()) => {}
            Err(err) => match err {
                super::ValidationError::ReservedBits(bits) => panic!(
                    "{:?} > {:?} > {:?} > {:?} : 0b{:b}",
                    index4, index3, index2, index1, bits
                ),
                super::ValidationError::NonCanonical(addr) => panic!(
                    "{:?} > {:?} > {:?} > {:?} : {:?}",
                    index4, index3, index2, index1, addr
                ),
            },
        };
    }
    pub fn validate_page_tables(&self) {
        debug!(
            "VIRTUAL ADDRESSOR: FULL VALIDATION: STARTED\n\tMAPPED: {:?}\tPML4: {:?}",
            self.mapped_offset(),
            self.pml4_page()
        );

        let mut validations: usize = 0;
        unsafe {
            let phys_mapped_addr = self.mapped_offset();
            for (index4, entry4) in self.pml4().iter().enumerate() {
                Self::validate_entry(Some(index4), None, None, None, entry4);
                validations += 1;

                if let Some(table3) = self.pml4().sub_table(index4, phys_mapped_addr) {
                    for (index3, entry3) in table3.iter().enumerate() {
                        Self::validate_entry(Some(index4), Some(index3), None, None, entry3);
                        validations += 1;

                        if let Some(table2) = table3.sub_table(index3, phys_mapped_addr) {
                            for (index2, entry2) in table2.iter().enumerate() {
                                Self::validate_entry(
                                    Some(index4),
                                    Some(index3),
                                    Some(index2),
                                    None,
                                    entry2,
                                );
                                validations += 1;

                                if let Some(table1) = table2.sub_table(index2, phys_mapped_addr) {
                                    for (index1, entry1) in table1.iter().enumerate() {
                                        Self::validate_entry(
                                            Some(index4),
                                            Some(index3),
                                            Some(index2),
                                            Some(index1),
                                            entry1,
                                        );
                                        validations += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        debug!(
            "VIRTUAL ADDRESSOR: FULL VALIDATION: COMPLETED ({} TOTAL VALIDATIONS)",
            validations
        );
    }

    pub fn validate_page_branch(&self, page: &Page) {
        debug!(
            "VIRTUAL ADDRESSOR: BRANCH VALIDATION: STARTED\n\tMAPPED: {:?}\tPML4: {:?}\n\tPAGE: {:?}",
            self.mapped_offset(),
            self.pml4_page(),
            page
        );

        let base_addr = page.base_addr();
        let p4_index = base_addr.p4_index();
        let p3_index = base_addr.p3_index();
        let p2_index = base_addr.p2_index();
        let p1_index = base_addr.p1_index();

        let entry4 = self.pml4().get_entry(p4_index);
        Self::validate_entry(Some(p4_index), None, None, None, entry4);
        debug!(
            "VIRTUAL ADDRESSOR: BRANCH VALIDATION:\n\tPTE4: {:?}",
            entry4
        );

        unsafe {
            let phys_mapped_addr = self.mapped_offset();

            if let Some(table3) = self.pml4().sub_table(p4_index, phys_mapped_addr) {
                let entry3 = table3.get_entry(p3_index);
                Self::validate_entry(Some(p4_index), Some(p3_index), None, None, entry3);
                debug!(
                    "VIRTUAL ADDRESSOR: BRANCH VALIDATION:\n\tPTE3: {:?}",
                    entry3
                );

                if let Some(table2) = table3.sub_table(p3_index, phys_mapped_addr) {
                    let entry2 = table2.get_entry(p2_index);
                    Self::validate_entry(
                        Some(p4_index),
                        Some(p3_index),
                        Some(p2_index),
                        None,
                        entry2,
                    );
                    debug!(
                        "VIRTUAL ADDRESSOR: BRANCH VALIDATION:\n\tPTE2: {:?}",
                        entry2
                    );

                    if let Some(table1) = table2.sub_table(p2_index, phys_mapped_addr) {
                        let entry1 = table1.get_entry(p1_index);
                        Self::validate_entry(
                            Some(p4_index),
                            Some(p3_index),
                            Some(p2_index),
                            Some(p1_index),
                            entry1,
                        );
                        debug!(
                            "VIRTUAL ADDRESSOR: BRANCH VALIDATION:\n\tPTE1: {:?}",
                            entry1
                        );
                    }
                }
            }
        }

        debug!("VIRTUAL ADDRESSOR: BRANCH VALIDATION: COMPLETED");
    }
}
