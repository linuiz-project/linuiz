use crate::memory::{
    global_lock_next, global_total,
    paging::{Level4, PageAttributes, PageTable, PageTableEntry},
    Frame, Page,
};
use x86_64::VirtAddr;

pub struct VirtualAddressor {
    mapped_page: Page,
    pml4_frame: Frame,
}

impl VirtualAddressor {
    /// Attempts to create a new VirtualAddressor, with `current_mapped_addr` specifying the current virtual
    /// address where the entirety of the system physical memory is mapped.
    ///
    /// Safety: this method is unsafe because `mapped_page` can be any value; that is, not necessarily
    /// a valid address in which physical memory is already mapped.
    pub unsafe fn new(mapped_page: Page) -> Self {
        Self {
            // we don't know where physical memory is mapped at this point,
            // so rely on what the caller specifies for us
            mapped_page,
            pml4_frame: global_lock_next()
                .expect("failed to lock frame for PML4 of VirtualAddressor"),
        }
    }

    fn pml4(&self) -> &PageTable<Level4> {
        unsafe {
            &*self
                .mapped_page
                .offset(self.pml4_frame.index())
                .addr()
                .as_ptr()
        }
    }

    fn pml4_mut(&mut self) -> &mut PageTable<Level4> {
        unsafe {
            &mut *self
                .mapped_page
                .offset(self.pml4_frame.index())
                .addr()
                .as_mut_ptr()
        }
    }

    fn get_page_entry(&self, page: &Page) -> Option<&PageTableEntry> {
        let offset = self.mapped_page.addr();
        let addr = (page.addr().as_u64() >> 12) as usize;

        unsafe {
            self.pml4()
                .sub_table((addr >> 27) & 0x1FF, offset)
                .and_then(|p3| p3.sub_table((addr >> 18) & 0x1FF, offset))
                .and_then(|p2| p2.sub_table((addr >> 9) & 0x1FF, offset))
                .and_then(|p1| Some(&p1[(addr >> 0) & 0x1FF]))
        }
    }

    fn get_page_entry_mut(&mut self, page: &Page) -> Option<&mut PageTableEntry> {
        let offset = self.mapped_page.addr();
        let addr = (page.addr().as_u64() >> 12) as usize;

        unsafe {
            self.pml4_mut()
                .sub_table_mut((addr >> 27) & 0x1FF, offset)
                .and_then(|p3| p3.sub_table_mut((addr >> 18) & 0x1FF, offset))
                .and_then(|p2| p2.sub_table_mut((addr >> 9) & 0x1FF, offset))
                .and_then(|p1| Some(&mut p1[(addr >> 0) & 0x1FF]))
        }
    }

    fn get_page_entry_create(&mut self, page: &Page) -> &mut PageTableEntry {
        let offset = self.mapped_page.addr();
        let addr = (page.addr().as_u64() >> 12) as usize;

        unsafe {
            &mut self
                .pml4_mut()
                .sub_table_create((addr >> 27) & 0x1FF, offset)
                .sub_table_create((addr >> 18) & 0x1FF, offset)
                .sub_table_create((addr >> 9) & 0x1FF, offset)[(addr >> 0) & 0x1FF]
        }
    }

    pub fn map(&mut self, page: &Page, frame: &Frame) {
        let entry = self.get_page_entry_create(page);
        if entry.is_present() {
            crate::instructions::tlb::invalidate(page);
        }
        entry.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);

        trace!("Mapped {:?}: {:?}", page, entry);
    }

    pub fn unmap(&mut self, page: &Page) {
        let entry = self.get_page_entry_create(page);
        entry.set_nonpresent();
        crate::instructions::tlb::invalidate(page);

        trace!("Unmapped {:?}: {:?}", page, entry);
    }

    pub fn identity_map(&mut self, frame: &Frame) {
        self.map(
            &Page::from_addr(VirtAddr::new(frame.addr().as_u64())),
            frame,
        );
    }

    pub fn is_mapped(&self, virt_addr: VirtAddr) -> bool {
        match self.get_page_entry(&Page::containing_addr(virt_addr)) {
            Some(entry) => entry.is_present(),
            None => false,
        }
    }

    pub fn is_mapped_to(&self, page: &Page, frame: &Frame) -> bool {
        match self.get_page_entry(page).and_then(|entry| entry.frame()) {
            Some(entry_frame) => frame.addr() == entry_frame.addr(),
            None => false,
        }
    }

    pub fn translate_page(&self, page: &Page) -> Option<Frame> {
        self.get_page_entry(page).and_then(|entry| entry.frame())
    }

    pub fn modify_mapped_page(&mut self, page: Page) {
        let total_memory_pages = global_total() / 0x1000;
        for index in 0..total_memory_pages {
            self.map(&page.offset(index), &Frame::from_index(index));
        }

        self.mapped_page = page;
    }

    #[inline(always)]
    pub unsafe fn swap_into(&self) {
        crate::registers::CR3::write(&self.pml4_frame, None);
    }
}
