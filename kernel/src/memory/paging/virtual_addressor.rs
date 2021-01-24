use crate::memory::{
    allocators::{global_memory, global_memory_mut},
    paging::{Level4, PageAttributes, PageTable, PageTableEntry},
    Frame, Page,
};
use spin::Mutex;
use x86_64::VirtAddr;

pub struct VirtualAddressorCell {
    addressor: Mutex<Option<VirtualAddressor>>,
}

impl VirtualAddressorCell {
    pub const fn empty() -> Self {
        Self {
            addressor: Mutex::new(None),
        }
    }

    pub fn init(&self, page: Page) {
        let mut inner = self.addressor.lock();

        if inner.is_some() {
            panic!("virtual addressor has already been confgiured");
        } else {
            let addressor = unsafe { VirtualAddressor::new(page) };
            core::mem::swap(&mut *inner, &mut Some(addressor));
        }
    }

    pub fn map(&self, page: &Page, frame: &Frame) {
        self.addressor
            .lock()
            .as_mut()
            .expect("virtual addressor has not been configured")
            .map(page, frame);
    }

    pub fn unmap(&self, page: &Page) {
        self.addressor
            .lock()
            .as_mut()
            .expect("virtual addressor has not been configured")
            .unmap(page);
    }

    pub fn identity_map(&self, frame: &Frame) {
        self.addressor
            .lock()
            .as_mut()
            .expect("virtual addressor has not been configured")
            .identity_map(frame);
    }

    pub fn is_mapped(&self, page: &Page) -> bool {
        self.addressor
            .lock()
            .as_mut()
            .expect("virtual addressor has not been configured")
            .is_mapped(page)
    }

    pub fn modify_mapped_page(&self, page: Page) {
        self.addressor
            .lock()
            .as_mut()
            .expect("virtual addressor has not been configured")
            .modify_mapped_page(page)
    }

    pub fn swap_into(&self) {
        self.addressor
            .lock()
            .as_mut()
            .expect("virtual addressor has not been configured")
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
            pml4_frame: global_memory_mut(|allocator| {
                allocator
                    .lock_next()
                    .expect("failed to lock frame for PML4 of VirtualAddressor")
            }),
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

    fn get_page_entry_mut(&mut self, page: &Page) -> Option<&mut PageTableEntry> {
        let offset = self.mapped_page.addr();
        let addr = (page.addr().as_u64() >> 12) as usize;

        self.pml4_mut()
            .sub_table_mut((addr >> 27) & 0x1FF, offset)
            .and_then(|p3| p3.sub_table_mut((addr >> 18) & 0x1FF, offset))
            .and_then(|p2| p2.sub_table_mut((addr >> 9) & 0x1FF, offset))
            .and_then(|p1| Some(&mut p1[(addr >> 0) & 0x1FF]))
    }

    fn get_page_entry_create(&mut self, page: &Page) -> &mut PageTableEntry {
        let offset = self.mapped_page.addr();
        let addr = (page.addr().as_u64() >> 12) as usize;

        &mut self
            .pml4_mut()
            .sub_table_create((addr >> 27) & 0x1FF, offset)
            .sub_table_create((addr >> 18) & 0x1FF, offset)
            .sub_table_create((addr >> 9) & 0x1FF, offset)[(addr >> 0) & 0x1FF]
    }

    fn map(&mut self, page: &Page, frame: &Frame) {
        let entry = self.get_page_entry_create(page);

        if entry.is_present() {
            crate::instructions::tlb::invalidate(page);
        }

        entry.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
        trace!("Mapped {:?} to {:?}: {:?}", page, entry.frame(), entry);
    }

    fn unmap(&mut self, page: &Page) {
        let entry = self.get_page_entry_create(page);

        entry.set_nonpresent();
        crate::instructions::tlb::invalidate(page);
        trace!("Unmapped {:?} from {:?}: {:?}", page, entry.frame(), entry);
    }

    fn identity_map(&mut self, frame: &Frame) {
        self.map(
            &Page::from_addr(VirtAddr::new(frame.addr().as_u64())),
            frame,
        );
    }

    fn is_mapped(&mut self, page: &Page) -> bool {
        self.get_page_entry_mut(page).is_some()
    }

    fn modify_mapped_page(&mut self, page: Page) {
        debug!(
            "Modifying mapped offset page: from {:?} to {:?}",
            self.mapped_page, page
        );

        let total_memory_pages =
            global_memory(|allocator| allocator.total_memory() / 0x1000) as u64;
        for index in 0..total_memory_pages {
            self.map(&page.offset(index), &Frame::from_index(index));
        }

        self.mapped_page = page;
    }

    fn swap_into(&self) {
        info!(
            "Writing virtual addressor's PML4 to CR3 register: {:?}.",
            self.pml4_frame
        );

        unsafe { crate::registers::CR3::write(&self.pml4_frame, None) };
    }
}
