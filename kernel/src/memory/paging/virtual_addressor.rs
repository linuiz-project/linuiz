use core::lazy::OnceCell;

use crate::memory::{
    global_lock_next, global_total,
    paging::{Level4, PageAttributes, PageTable, PageTableEntry},
    Frame, Page,
};
use spin::Mutex;
use x86_64::VirtAddr;

static VADDR_NOT_CFG: &str = "virtual addressor has not been configured";

pub struct VirtualAddressorCell {
    addressor: Mutex<OnceCell<VirtualAddressor>>,
}

impl VirtualAddressorCell {
    pub const fn empty() -> Self {
        Self {
            addressor: Mutex::new(OnceCell::new()),
        }
    }

    pub fn init(&self, page: Page) {
        let virtual_addressor = unsafe { VirtualAddressor::new(page) };
        if let Err(_) = self.addressor.lock().set(virtual_addressor) {
            panic!(VADDR_NOT_CFG);
        }
    }

    pub fn map(&self, page: &Page, frame: &Frame) {
        self.addressor
            .lock()
            .get_mut()
            .expect(VADDR_NOT_CFG)
            .map(page, frame);
    }

    pub fn unmap(&self, page: &Page) {
        self.addressor
            .lock()
            .get_mut()
            .expect(VADDR_NOT_CFG)
            .unmap(page);
    }

    pub fn identity_map(&self, frame: &Frame) {
        self.addressor
            .lock()
            .get_mut()
            .expect(VADDR_NOT_CFG)
            .identity_map(frame);
    }

    pub fn is_mapped(&self, virt_addr: VirtAddr) -> bool {
        self.addressor
            .lock()
            .get()
            .expect(VADDR_NOT_CFG)
            .is_mapped(virt_addr)
    }

    pub fn is_mapped_to(&self, page: &Page, frame: &Frame) -> bool {
        self.addressor
            .lock()
            .get()
            .expect(VADDR_NOT_CFG)
            .is_mapped_to(page, frame)
    }

    pub fn modify_mapped_page(&self, page: Page) {
        self.addressor
            .lock()
            .get_mut()
            .expect(VADDR_NOT_CFG)
            .modify_mapped_page(page)
    }

    pub unsafe fn swap_into(&self) {
        self.addressor
            .lock()
            .get()
            .expect(VADDR_NOT_CFG)
            .swap_into()
    }
}

struct VirtualAddressor {
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
        debug!(
            "Attempting to create a VirtualAddressor (current mapped address supplied: {:?}).",
            mapped_page
        );

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

    fn map(&mut self, page: &Page, frame: &Frame) {
        let entry = self.get_page_entry_create(page);
        if entry.is_present() {
            crate::instructions::tlb::invalidate(page);
        }
        entry.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);

        trace!("Mapped {:?}: {:?}", page, entry);
    }

    fn unmap(&mut self, page: &Page) {
        let entry = self.get_page_entry_create(page);
        entry.set_nonpresent();
        crate::instructions::tlb::invalidate(page);

        trace!("Unmapped {:?}: {:?}", page, entry);
    }

    fn identity_map(&mut self, frame: &Frame) {
        self.map(
            &Page::from_addr(VirtAddr::new(frame.addr().as_u64())),
            frame,
        );
    }

    fn is_mapped(&self, virt_addr: VirtAddr) -> bool {
        match self.get_page_entry(&Page::containing_addr(virt_addr)) {
            Some(entry) => entry.is_present(),
            None => false,
        }
    }

    fn is_mapped_to(&self, page: &Page, frame: &Frame) -> bool {
        match self.get_page_entry(page).and_then(|entry| entry.frame()) {
            Some(entry_frame) => frame.addr() == entry_frame.addr(),
            None => false,
        }
    }

    fn modify_mapped_page(&mut self, page: Page) {
        let total_memory_pages = (global_total() / 0x1000) as u64;
        for index in 0..total_memory_pages {
            self.map(&page.offset(index), &Frame::from_index(index));
        }

        self.mapped_page = page;
    }

    #[inline(always)]
    unsafe fn swap_into(&self) {
        crate::registers::CR3::write(&self.pml4_frame, None);
    }
}
