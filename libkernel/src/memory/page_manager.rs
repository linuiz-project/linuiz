use crate::{
    instructions::tlb,
    memory::{
        frame_manager::{FrameManager, FrameOwnership},
        paging::{AttributeModify, Level4, PageAttribute, PageTable, PageTableEntry},
        Page,
    },
};
use libarch::{instructions::interrupts::without_interrupts, Address, Physical, Virtual};

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
    /// Attempts to create a new PageManager, with `mapped_page` specifying the current page
    /// where the entirety of the system physical memory is mapped.
    ///
    /// SAFETY: This method is unsafe because `mapped_page` can be any value; that is, not necessarily
    ///         a valid address in which physical memory is already mapped. The expectation is that `mapped_page`
    ///         is a proper starting page for the current physical memory mapping.
    pub unsafe fn new(mapped_page: &Page, pml4_index: usize) -> Self {
        Self {
            // We don't know where physical memory is mapped at this point,
            // so rely on what the caller specifies for us.
            mapped_page: *mapped_page,
            pml4_frame: pml4_index,
        }
    }

    fn mapped_offset(&self) -> Address<Virtual> {
        self.mapped_page.base_addr()
    }

    /* ACQUIRE STATE */

    fn pml4_page(&self) -> Page {
        self.mapped_page.forward_checked(self.pml4_frame).unwrap()
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
                .and_then(|p4| p4.sub_table(addr.p4_index(), &mapped_page))
                .and_then(|p3| p3.sub_table(addr.p3_index(), &mapped_page))
                .and_then(|p2| p2.sub_table(addr.p2_index(), &mapped_page))
                .and_then(|p1| Some(p1.get_entry(addr.p1_index())))
        }
    }

    fn get_page_entry_mut(&mut self, page: &Page) -> Option<&mut PageTableEntry> {
        let mapped_page = self.mapped_page;
        let addr = page.base_addr();

        unsafe {
            self.pml4_mut()
                .and_then(|p4| p4.sub_table_mut(addr.p4_index(), &mapped_page))
                .and_then(|p3| p3.sub_table_mut(addr.p3_index(), &mapped_page))
                .and_then(|p2| p2.sub_table_mut(addr.p2_index(), &mapped_page))
                .and_then(|p1| Some(p1.get_entry_mut(addr.p1_index())))
        }
    }

    fn get_page_entry_create(&mut self, page: &Page, frame_manager: &'static FrameManager<'_>) -> &mut PageTableEntry {
        let mapped_page = self.mapped_page;
        let addr = page.base_addr();

        // TODO abstract this into the source of page manager

        unsafe {
            self.pml4_mut()
                .unwrap()
                .sub_table_create(addr.p4_index(), &mapped_page, frame_manager)
                .sub_table_create(addr.p3_index(), &mapped_page, frame_manager)
                .sub_table_create(addr.p2_index(), &mapped_page, frame_manager)
                .get_entry_mut(addr.p1_index())
        }
    }

    unsafe fn modify_mapped_page(&mut self, base_page: Page, frame_manager: &'static FrameManager) {
        for frame_index in 0..frame_manager.total_frames() {
            let cur_page = base_page.forward_checked(frame_index).unwrap();

            self.get_page_entry_create(&cur_page, frame_manager).set(
                frame_index,
                PageAttribute::PRESENT
                    | PageAttribute::WRITABLE
                    | PageAttribute::WRITE_THROUGH
                    | PageAttribute::NO_EXECUTE
                    | PageAttribute::GLOBAL,
            );

            tlb::invlpg(&cur_page);
        }

        self.mapped_page = base_page;
    }

    /// Returns `true` if the current CR3 address matches the addressor's PML4 frame, and `false` otherwise.
    fn is_active(&self) -> bool {
        libarch::registers::x86_64::control::CR3::read().0.frame_index() == self.pml4_frame
    }

    #[inline(always)]
    pub unsafe fn write_cr3(&mut self) {
        libarch::registers::x86_64::control::CR3::write(
            Address::<Physical>::new(self.pml4_frame * 0x1000),
            libarch::registers::x86_64::control::CR3Flags::empty(),
        );
    }
}

pub struct PageManager {
    virtual_map: spin::RwLock<VirtualMapper>,
}

unsafe impl Send for PageManager {}
unsafe impl Sync for PageManager {}

impl PageManager {
    /// SAFETY: Refer to `VirtualMapper::new()`.
    pub unsafe fn new(
        frame_manager: &'static FrameManager<'_>,
        mapped_page: &Page,
        pml4_copy: Option<PageTable<Level4>>,
    ) -> Self {
        Self {
            virtual_map: spin::RwLock::new({
                let pml4_index = frame_manager.lock_next().expect("Failed to lock frame for virtual addressor's PML4");

                if let Some(pml4) = pml4_copy {
                    // Write existing PML4.
                    mapped_page
                        .forward_checked(pml4_index)
                        .unwrap()
                        .as_mut_ptr::<PageTable<Level4>>()
                        .write_volatile(pml4);
                } else {
                    // Clear PML4 frame.
                    core::ptr::write_bytes(
                        mapped_page.forward_checked(pml4_index).unwrap().as_mut_ptr::<u8>(),
                        0,
                        0x1000,
                    );
                }

                VirtualMapper::new(mapped_page, pml4_index)
            }),
        }
    }

    pub fn cr3(&self) -> usize {
        self.virtual_map.read().pml4_frame * 0x1000
    }

    pub unsafe fn from_current(mapped_page: &Page) -> Self {
        Self {
            virtual_map: spin::RwLock::new(VirtualMapper::new(
                mapped_page,
                libarch::registers::x86_64::control::CR3::read().0.frame_index(),
            )),
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
        ownership: FrameOwnership,
        attribs: PageAttribute,
        frame_manager: &'static FrameManager<'_>,
    ) -> Result<(), MapError> {
        without_interrupts(|| {
            // Lock the virtual map first. This avoids a situation where the frame for this page is
            // freed, an interrupt occurs, and then the page is memory referenced (and thus, a page
            // pointing to a frame it doesn't own is accessed).
            let mut map_write = self.virtual_map.write();

            // Attempt to acquire the requisite frame, following the outlined parsing of `lock_frame`.
            let frame_result = match ownership {
                FrameOwnership::None => Ok(frame_index),
                FrameOwnership::Borrowed => frame_manager.borrow(frame_index),
                FrameOwnership::Locked => frame_manager.lock(frame_index),
            };

            match frame_result {
                // If acquisition of the frame is successful, map the page to the frame index.
                Ok(frame_index) => {
                    map_write.get_page_entry_create(page, frame_manager).set(frame_index, attribs);

                    tlb::invlpg(page);

                    Ok(())
                }

                // If the acquisition of the frame fails, return the error.
                Err(err) => Err(MapError::FrameError(err)),
            }
        })
    }

    /// Unmaps the given page, optionally freeing / dropping the frame the page points to within the given [`FrameManager`].
    pub fn unmap(
        &self,
        page: &Page,
        ownership: FrameOwnership,
        frame_manager: &'static FrameManager<'_>,
    ) -> Result<(), MapError> {
        without_interrupts(|| {
            self.virtual_map
                .write()
                .get_page_entry_mut(page)
                .map(|entry| {
                    entry.set_attributes(PageAttribute::PRESENT, AttributeModify::Remove);

                    // Handle frame permissions to keep them updated.
                    unsafe {
                        match ownership {
                            FrameOwnership::None => {}
                            FrameOwnership::Borrowed => frame_manager.drop(entry.take_frame_index()).unwrap(),
                            FrameOwnership::Locked => frame_manager.free(entry.take_frame_index()).unwrap(),
                        }
                    }

                    // Invalidate the page in the TLB.
                    tlb::invlpg(page);
                })
                .ok_or(MapError::NotMapped)
        })
    }

    pub fn copy_by_map(
        &self,
        unmap_from: &Page,
        map_to: &Page,
        new_attribs: Option<PageAttribute>,
        frame_manager: &'static FrameManager<'_>,
    ) -> Result<(), MapError> {
        without_interrupts(|| {
            // Lock virtual map first. See blurp about the same potential UB condition from `Self::map()`.
            let mut map_write = self.virtual_map.write();

            let maybe_attribs_frame_index = {
                map_write.get_page_entry_mut(unmap_from).map(|entry| {
                    let attribs = new_attribs.unwrap_or_else(|| entry.get_attribs());
                    entry.set_attributes(PageAttribute::empty(), AttributeModify::Set);

                    unsafe { (attribs, entry.take_frame_index()) }
                })
            };

            match maybe_attribs_frame_index.ok_or(MapError::NotMapped) {
                Ok((attribs, frame_index)) => {
                    // Yes, we effectively completely re-implement the `Self::map()` function.
                    // This is primarily to ensure the `Self::map()` implementation is kept clean, as this
                    // function requires locking `self.virtual_map`, and so calling `Self::map()` would deadlock.
                    //
                    // Rather than add non-locking functionality to `Self::map()`, we keep intent clear here and
                    // mostly re-implement the function, slightly modified.

                    // Attempt to acquire the requisite frame, following the outlined parsing of `lock_frame`.
                    let frame_result = match ownership {
                        FrameOwnership::None => Ok(frame_index),
                        FrameOwnership::Borrowed => frame_manager.borrow(frame_index),
                        FrameOwnership::Locked => frame_manager.lock(frame_index),
                    };

                    match frame_result {
                        // If acquisition of the frame is successful, map the page to the frame index and invalidate the TLB
                        // entries for the to/from pages.
                        Ok(frame_index) => {
                            map_write.get_page_entry_create(map_to, frame_manager).set(frame_index, attribs);

                            tlb::invlpg(map_to);
                            tlb::invlpg(unmap_from);

                            Ok(())
                        }

                        // If the acquisition of the frame fails, return the error.
                        Err(err) => Err(MapError::FrameError(err)),
                    }
                }
                Err(err) => Err(err),
            }
        })
    }

    pub fn auto_map(&self, page: &Page, attribs: PageAttribute, frame_manager: &'static FrameManager<'_>) {
        self.map(page, frame_manager.lock_next().unwrap(), FrameOwnership::None, attribs, frame_manager).unwrap();
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, virt_addr: Address<Virtual>) -> bool {
        without_interrupts(|| {
            self.virtual_map
                .read()
                .get_page_entry(&Page::containing_addr(virt_addr))
                .filter(|entry| entry.get_attribs().contains(PageAttribute::PRESENT))
                .is_some()
        })
    }

    pub fn is_mapped_to(&self, page: &Page, frame_index: usize) -> bool {
        without_interrupts(|| {
            self.virtual_map
                .read()
                .get_page_entry(page)
                .and_then(|entry| entry.get_frame_index())
                .map_or(false, |entry_frame_index| frame_index == entry_frame_index)
        })
    }

    pub fn get_mapped_to(&self, page: &Page) -> Option<usize> {
        without_interrupts(|| self.virtual_map.read().get_page_entry(page).and_then(|entry| entry.get_frame_index()))
    }

    /* STATE CHANGING */

    pub fn get_page_attribs(&self, page: &Page) -> Option<PageAttribute> {
        without_interrupts(|| self.virtual_map.read().get_page_entry(page).map(|page_entry| page_entry.get_attribs()))
    }

    pub unsafe fn set_page_attribs(&self, page: &Page, attributes: PageAttribute, modify_mode: AttributeModify) {
        without_interrupts(|| {
            self.virtual_map
                .write()
                .get_page_entry_mut(page)
                .map(|page_entry| page_entry.set_attributes(attributes, modify_mode));

            tlb::invlpg(page);
        })
    }

    pub unsafe fn modify_mapped_page(&self, page: Page, frame_manager: &'static FrameManager<'_>) {
        without_interrupts(|| {
            self.virtual_map.write().modify_mapped_page(page, frame_manager);
        })
    }

    pub fn mapped_page(&self) -> Page {
        self.virtual_map.read().mapped_page
    }

    #[inline(always)]
    pub unsafe fn write_cr3(&self) {
        without_interrupts(|| {
            self.virtual_map.write().write_cr3();
        })
    }

    pub fn copy_pml4(&self) -> PageTable<Level4> {
        without_interrupts(|| {
            let vmap = self.virtual_map.read();

            unsafe {
                vmap.mapped_page.forward_checked(vmap.pml4_frame).unwrap().as_ptr::<PageTable<Level4>>().read_volatile()
            }
        })
    }
}
