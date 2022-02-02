use core::mem::MaybeUninit;

use spin::RwLock;

use crate::{
    memory::{
        paging::{AttributeModify, Level4, PageAttributes, PageTable, PageTableEntry},
        Page, FRAME_MANAGER,
    },
    Address, {Physical, Virtual},
};

#[derive(Debug, Clone, Copy)]
pub enum MapError {
    NotMapped,
    AlreadyMapped,
    FrameError(crate::memory::FrameError),
}

struct VirtualMapper {
    mapped_page: Page,
    pml4_frame: usize,
}

impl VirtualMapper {
    pub const fn uninit() -> MaybeUninit<Self> {
        MaybeUninit::new(Self {
            mapped_page: Page::null(),
            pml4_frame: usize::MAX,
        })
    }

    /// Attempts to create a new PageManager, with `mapped_page` specifying the current page
    /// where the entirety of the system physical memory is mapped.
    ///
    /// SAFETY: this method is unsafe because `mapped_page` can be any value; that is, not necessarily
    /// a valid address in which physical memory is already mapped. The expectation is that `mapped_page` is
    /// a proper starting page for the physical memory mapping.
    pub unsafe fn new(mapped_page: &Page) -> Self {
        let pml4_index = FRAME_MANAGER
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
            mapped_page: *mapped_page,
            pml4_frame: pml4_index,
        }
    }

    pub fn mapped_offset(&self) -> Address<Virtual> {
        self.mapped_page.base_addr()
    }

    /* ACQUIRE STATE */

    fn pml4_page(&self) -> Page {
        self.mapped_page.forward(self.pml4_frame).unwrap()
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

    pub unsafe fn modify_mapped_page(&mut self, page: Page) {
        for frame_index in 0..FRAME_MANAGER.total_frame_count() {
            let cur_page = page.forward(frame_index).unwrap();

            self.get_page_entry_create(&cur_page).set(
                frame_index,
                PageAttributes::DATA_BITS | PageAttributes::GLOBAL,
            );

            crate::instructions::tlb::invalidate(&cur_page);
        }

        self.mapped_page = page;
    }

    /// Returns `true` if the current CR3 address matches the addressor's PML4 frame, and `false` otherwise.
    pub fn is_active(&self) -> bool {
        crate::registers::CR3::read().0.frame_index() == self.pml4_frame
    }

    pub unsafe fn write_cr3(&self) {
        crate::registers::CR3::write(
            Address::<Physical>::new(self.pml4_frame * 0x1000),
            crate::registers::CR3Flags::empty(),
        );
    }

    // TODO
    // fn copy_physical_memory_entries(page_manager: &mut PageManager) {
    //     let k_vmap = PAGE_MANAGER.virtual_mapper.read();
    //     let mut cur_vmap = page_manager.virtual_mapper.write();

    //     let k_pml4 = k_vmap
    //         .pml4()
    //         .expect("Kernel page manager has no valid PML4.");
    //     let cur_pml4 = cur_vmap
    //         .pml4_mut()
    //         .expect("Provided page manager has no valid PML4.");

    //     for entry_index in k_vmap.mapped_offset().p4_index()..512 {
    //         *cur_pml4.get_entry_mut(entry_index) = *k_pml4.get_entry(entry_index);
    //     }
    // }
}

pub struct PageManager {
    virtual_mapper: RwLock<VirtualMapper>,
}

impl PageManager {
    pub const fn uninit() -> MaybeUninit<Self> {
        MaybeUninit::new(Self {
            virtual_mapper: RwLock::new(unsafe { VirtualMapper::uninit().assume_init() }),
        })
    }

    /// SAFETY: Refer to `VirtualMapper::new()`.
    pub unsafe fn new(mapped_page: &Page) -> Self {
        Self {
            virtual_mapper: RwLock::new(VirtualMapper::new(mapped_page)),
        }
    }

    /* MAP / UNMAP */

    /// Maps the specified frame to the specified frame index.
    ///
    /// The `lock_frame` parameter is set as follows:
    ///     `Some` or `None` indicates whether frame allocation is required,
    ///         `true` or `false` in a `Some` indicates whether the frame is locked or merely referenced.
    pub fn map(
        &self,
        page: &Page,
        frame_index: usize,
        lock_frame: Option<bool>,
        attribs: PageAttributes,
    ) -> Result<(), MapError> {
        // Attempt to acquire the requisite frame, following the outlined parsing of `lock_frame`.
        match lock_frame {
            Some(true) => FRAME_MANAGER.lock(frame_index),
            Some(false) => FRAME_MANAGER.borrow(frame_index),
            None => Ok(frame_index),
        }
        // If the acquisition of the frame fails, transform the error.
        .map_err(|falloc_err| MapError::FrameError(falloc_err))
        // If acquisition of the frame is successful, map the page to the frame index.
        .map(|frame_index| {
            self.virtual_mapper
                .write()
                .get_page_entry_create(page)
                .set(frame_index, attribs);

            crate::instructions::tlb::invalidate(page);
        })
    }

    pub fn unmap(&self, page: &Page, locked: bool) -> Result<(), MapError> {
        self.virtual_mapper
            .write()
            .get_page_entry_mut(page)
            .map(|entry| {
                entry.set_attributes(PageAttributes::PRESENT, AttributeModify::Remove);

                // Drop pointed frame since the entry is no longer present.
                unsafe {
                    if locked {
                        FRAME_MANAGER.free(entry.take_frame_index()).unwrap();
                    } else {
                        FRAME_MANAGER.drop(entry.take_frame_index()).unwrap();
                    }
                }

                // Invalidate the page in the TLB.
                crate::instructions::tlb::invalidate(page);
            })
            .ok_or(MapError::NotMapped)
    }

    pub fn copy_by_map(
        &self,
        unmap_from: &Page,
        map_to: &Page,
        new_attribs: Option<PageAttributes>,
    ) -> Result<(), MapError> {
        let frame_index = {
            self.virtual_mapper
                .write()
                .get_page_entry_mut(unmap_from)
                .map(|entry| {
                    let attribs = new_attribs.unwrap_or_else(|| entry.get_attribs());
                    entry.set_attributes(PageAttributes::empty(), AttributeModify::Set);
                    unsafe { (attribs, entry.take_frame_index()) }
                })
        };

        frame_index
            .ok_or(MapError::NotMapped)
            .map(|(attribs, frame_index)| {
                self.map(map_to, frame_index, None, attribs).unwrap();
                crate::instructions::tlb::invalidate(unmap_from);
            })
    }

    pub fn auto_map(&self, page: &Page, attribs: PageAttributes) {
        self.map(page, FRAME_MANAGER.lock_next().unwrap(), None, attribs)
            .unwrap();
    }

    /// Attempts to create a 1:1 mapping between a virual memory page and its physical counterpart.
    ///
    /// REMARK:
    ///     This function assumes the frame for the identity mapping is:
    ///         A) a valid lock or borrow
    ///         B) not required for the mapping
    pub fn identity_map(&self, page: &Page, attribs: PageAttributes) -> Result<(), MapError> {
        self.map(page, page.index(), None, attribs)
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, virt_addr: Address<Virtual>) -> Option<usize> {
        self.virtual_mapper
            .read()
            .get_page_entry(&Page::containing_addr(virt_addr))
            .filter(|entry| entry.get_attribs().contains(PageAttributes::PRESENT))
            .and_then(|entry| entry.get_frame_index())
    }

    pub fn is_mapped_to(&self, page: &Page, frame_index: usize) -> bool {
        let vmap = self.virtual_mapper.read();

        match vmap
            .get_page_entry(page)
            .and_then(|entry| entry.get_frame_index())
        {
            Some(entry_frame_index) => frame_index == entry_frame_index,
            None => false,
        }
    }

    /* STATE CHANGING */

    pub fn get_page_attribs(&self, page: &Page) -> Option<PageAttributes> {
        self.virtual_mapper
            .read()
            .get_page_entry(page)
            .map(|page_entry| page_entry.get_attribs())
    }

    pub unsafe fn set_page_attribs(
        &self,
        page: &Page,
        attributes: PageAttributes,
        modify_mode: AttributeModify,
    ) {
        self.virtual_mapper
            .write()
            .get_page_entry_mut(page)
            .map(|page_entry| page_entry.set_attributes(attributes, modify_mode));
    }

    pub unsafe fn modify_mapped_page(&self, page: Page) {
        self.virtual_mapper.write().modify_mapped_page(page);
    }

    pub fn mapped_addr(&self) -> Address<Virtual> {
        self.virtual_mapper.read().mapped_page.base_addr()
    }

    pub fn write_cr3(&self) {
        unsafe {
            crate::registers::CR3::write(
                Address::<Physical>::new(self.virtual_mapper.read().pml4_frame * 0x1000),
                crate::registers::CR3Flags::empty(),
            )
        };
    }
}

static UNINIT_PAGE_MANAGER: PageManager = unsafe { PageManager::uninit().assume_init() };

static mut PAGE_MANAGER: &'static PageManager = &UNINIT_PAGE_MANAGER;

pub unsafe fn set_page_manager(page_manager: &'static PageManager) {
    PAGE_MANAGER = page_manager;
}

pub fn get_page_manager() -> &'static PageManager {
    unsafe { PAGE_MANAGER }
}
