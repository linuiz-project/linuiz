use crate::{
    addr_ty::{Physical, Virtual},
    memory::{
        paging::{AttributeModify, Level4, PageAttributes, PageTable, PageTableEntry},
        Frame, Page,
    },
    Address,
};

#[derive(Debug, Clone, Copy)]
pub enum MapError {
    AlreadyMapped,
    AlreadyUnmapped,
}

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

    fn pml4(&self) -> Option<&PageTable<Level4>> {
        unsafe { self.pml4_page().as_ptr::<PageTable<Level4>>().as_ref() }
    }

    fn pml4_mut(&mut self) -> Option<&mut PageTable<Level4>> {
        unsafe { self.pml4_page().as_mut_ptr::<PageTable<Level4>>().as_mut() }
    }

    fn get_page_entry(&self, page: &Page) -> Option<&PageTableEntry> {
        let offset = self.mapped_page.base_addr();
        let addr = page.base_addr();

        unsafe {
            self.pml4()
                .and_then(|p4| p4.sub_table(addr.p4_index(), offset))
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
                .and_then(|p4| p4.sub_table_mut(addr.p4_index(), offset))
                .and_then(|p3| p3.sub_table_mut(addr.p3_index(), offset))
                .and_then(|p2| p2.sub_table_mut(addr.p2_index(), offset))
                .and_then(|p1| Some(p1.get_entry_mut(addr.p1_index())))
        }
    }

    fn get_page_entry_create(&mut self, page: &Page) -> &mut PageTableEntry {
        let offset = self.mapped_page.base_addr();
        let addr = page.base_addr();

        unsafe {
            self.pml4_mut().unwrap()
                .sub_table_create(addr.p4_index(), offset)
                .sub_table_create(addr.p3_index(), offset)
                .sub_table_create(addr.p2_index(), offset)
                .get_entry_mut(addr.p1_index())
        }
    }

    /* MAP / UNMAP */

    pub fn map(&mut self, page: &Page, frame: &Frame) -> Result<(), MapError> {
        if self.is_mapped(page.base_addr()) {
            return Err(MapError::AlreadyMapped);
        }

        self.get_page_entry_create(page)
            .set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
        crate::instructions::tlb::invalidate(page);

        Ok(())
    }

    pub fn unmap(&mut self, page: &Page) -> Result<(), MapError> {
        if !self.is_mapped(page.base_addr()) {
            return Err(MapError::AlreadyUnmapped);
        }

        self.get_page_entry_mut(page)
            .unwrap()
            .set_attributes(PageAttributes::PRESENT, AttributeModify::Remove);
        crate::instructions::tlb::invalidate(page);

        Ok(())
    }

    pub fn identity_map(&mut self, frame: &Frame) -> Result<(), MapError> {
        self.map(&Page::from_index(frame.index()), frame)
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
            self.map(&page.forward(index).unwrap(), &Frame::from_index(index))
                .unwrap();
        }

        self.mapped_page = page;
    }

    pub unsafe fn get_page_attribs(&self, page: &Page) -> Option<PageAttributes> {
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
        trace!("Swapping {:?} into CR3.", self.pml4_frame.base_addr());
        crate::registers::CR3::write(&self.pml4_frame, crate::registers::CR3Flags::empty());
    }
}
