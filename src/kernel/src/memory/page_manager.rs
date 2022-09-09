use crate::{
    interrupts,
    memory::{AttributeModify, FrameManager, Mut, PageAttributes, PageTable, PageTableEntry, Ref},
};
use libkernel::{memory::Page, Address, Physical};
use spin::RwLock;

use super::VmemRegister;

#[derive(Debug, Clone, Copy)]
pub enum PagingError {
    NotMapped,
    AlreadyMapped,
    FrameError(crate::memory::FrameError),
    InvalidRootFrame,
}

struct PageManagerData {
    depth: usize,
    root_frame_index: usize,
    phys_mapped_page: Page,
    entry: PageTableEntry,
}

pub struct PageManager(RwLock<PageManagerData>);

// SAFETY: Type is designed to be thread-agnostic internally.
unsafe impl Sync for PageManager {}

impl PageManager {
    /// Attempts to construct a new page manager. Returns `None` if the provided page table depth is not supported.
    /// SAFETY: Refer to `VirtualMapper::new()`.
    pub unsafe fn new(
        depth: usize,
        phys_mapped_page: &Page,
        vmem_register_copy: Option<VmemRegister>,
        frame_manager: &'static FrameManager<'_>,
    ) -> Option<Self> {
        const VALID_DEPTHS: core::ops::RangeInclusive<usize> = 3..=5;

        if VALID_DEPTHS.contains(&depth)
            && let Ok(root_frame_index) = frame_manager.lock_next()
            && let Some(root_mapped_page) = phys_mapped_page.forward_checked(root_frame_index)
        {
            match vmem_register_copy {
                Some(vmem_register_copy) => {
                    let copy_mapped_page = phys_mapped_page.forward_checked(vmem_register_copy.0.frame_index())?;
                    core::ptr::copy_nonoverlapping(copy_mapped_page.address().as_ptr::<u8>(), root_mapped_page.address().as_mut_ptr::<u8>(), 0x1000);
                },
                None => core::ptr::write_bytes(root_mapped_page.address().as_mut_ptr::<u8>(), 0, 0x1000),
            }

            Some(Self(RwLock::new(PageManagerData { depth, root_frame_index, phys_mapped_page: *phys_mapped_page, entry: PageTableEntry::new(root_frame_index, PageAttributes::PRESENT) })))
        } else {
            None
        }
    }

    pub unsafe fn from_current(phys_mapped_page: &Page) -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let root_frame_index = crate::arch::x64::registers::control::CR3::read().0.frame_index();
            let root_table_entry = PageTableEntry::new(root_frame_index, PageAttributes::PRESENT);
            if let Some(ext_feature_info) = crate::arch::x64::cpu::cpuid::EXT_FEATURE_INFO.as_ref()
                && ext_feature_info.has_la57()
                && crate::arch::x64::registers::control::CR4::read().contains(crate::arch::x64::registers::control::CR4Flags::LA57)
            {
                Self(RwLock::new(PageManagerData { depth: 5, root_frame_index, phys_mapped_page:* phys_mapped_page, entry: root_table_entry }))
            } else {
                Self(RwLock::new(PageManagerData { depth: 4, root_frame_index, phys_mapped_page:* phys_mapped_page, entry: root_table_entry }))
            }
        }

        #[cfg(target_arch = "riscv64")]
        {
            // TODO
            let root_frame_index = crate::arch::rv64::registers::satp::get_ppn();
        }
    }

    fn with_root_table<T>(&self, func: impl FnOnce(PageTable<Ref>) -> T) -> T {
        interrupts::without(|| {
            let data = self.0.read();
            // SAFETY: This type requires that the physical mapping page is valid, so it can be safely passed to the page table.
            func(unsafe { PageTable::<Ref>::new(data.depth, data.phys_mapped_page, &data.entry) })
        })
    }

    fn with_root_table_mut<T>(&self, func: impl FnOnce(PageTable<Mut>) -> T) -> T {
        interrupts::without(|| {
            let mut data = self.0.write();
            // SAFETY: This type requires that the physical mapping page is valid, so it can be safely passed to the page table.
            func(unsafe { PageTable::<Mut>::new(data.depth, data.phys_mapped_page, &mut data.entry) })
        })
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the specified frame index.
    pub fn map(
        &self,
        page: &Page,
        frame_index: usize,
        lock_frame: bool,
        attributes: PageAttributes,
        frame_manager: &'static FrameManager<'_>,
    ) -> Result<(), PagingError> {
        let result = self.with_root_table_mut(|mut root_table| {
            match if lock_frame { frame_manager.lock(frame_index) } else { Ok(frame_index) } {
                // If acquisition of the frame is successful, attempt to map the page to the frame index.
                Ok(frame_index)
                    // Only return `Ok(())` if the following closure returns `Some(())` (meaning the page's table walk succeeded and was modified).
                    if root_table
                        .with_entry_create(page, frame_manager, |entry| {
                            // SAFETY: We've got an explicit directive from the caller to map these pages, and we've checked the condition of the
                            //         pages and entries, so if this isn't safe it's on the caller.
                            unsafe {
                                entry.set_frame_index(frame_index);
                                entry.set_attributes(attributes, AttributeModify::Set);
                            }

                            #[cfg(target_arch = "x86_64")]
                            crate::arch::x64::instructions::tlb::invlpg(page);
                        })
                        .is_some() =>
                {
                    Ok(())
                }

                Ok(_) => Err(PagingError::NotMapped),

                // If the acquisition of the frame fails, return the error.
                Err(err) => Err(PagingError::FrameError(err)),
            }
        });

        #[cfg(debug_assertions)]
        if result.is_ok() {
            debug_assert_eq!(self.get_mapped_to(page), Some(frame_index));
            debug_assert_eq!(self.get_page_attributes(page), Some(attributes));
        }

        result
    }

    /// Helper function to map MMIO pages and update `FrameManager` state.
    ///
    /// SAFETY: This function trusts implicitly that the provided page and frame index are valid for mapping.
    pub unsafe fn map_mmio(
        &self,
        page: &Page,
        frame_index: usize,
        frame_manager: &'static FrameManager<'_>,
    ) -> Result<(), PagingError> {
        frame_manager.lock(frame_index).ok();
        frame_manager.force_modify_type(frame_index, crate::memory::FrameType::MMIO).ok();

        self.set_page_attributes(page, PageAttributes::MMIO, AttributeModify::Set)
            .or_else(|_| self.map(&page, frame_index, false, PageAttributes::MMIO, frame_manager))
    }

    /// Unmaps the given page, optionally freeing the frame the page points to within the given [`FrameManager`].
    ///
    /// SAFETY: Caller must ensure calling this function does not cause memory corruption.
    pub unsafe fn unmap(
        &self,
        page: &Page,
        free_frame: bool,
        frame_manager: &'static FrameManager<'_>,
    ) -> Result<(), PagingError> {
        self.with_root_table_mut(|mut root_table| {
            match root_table.with_entry_mut(page, |entry| {
                // SAFETY: We've got an explicit directive from the caller to unmap this page, so the caller must ensure that's a valid operation.
                unsafe { entry.set_attributes(PageAttributes::PRESENT, AttributeModify::Remove) };

                // Handle frame permissions to keep them updated.
                if free_frame {
                    let frame_index = entry.get_frame_index();

                    // SAFETY: See above.
                    unsafe {
                        entry.set_frame_index(0);
                        frame_manager.free(frame_index).unwrap();
                    }
                }

                // Invalidate the page in the TLB.
                #[cfg(target_arch = "x86_64")]
                crate::arch::x64::instructions::tlb::invlpg(page);
            }) {
                Some(()) => Ok(()),
                None => Err(PagingError::NotMapped),
            }
        })
    }

    // pub fn copy_by_map(
    //     &self,
    //     unmap_from: &Page,
    //     map_to: &Page,
    //     new_attribs: Option<PageAttributes>,
    //     frame_manager: &'static FrameManager<'_>,
    // ) -> Result<(), MapError> {
    //     interrupts::without(|| {
    //         let mut map_write = self.0.write();

    //         let maybe_new_pte_frame_index_attribs = map_write.get_page_entry_mut(unmap_from).map(|entry| {
    //             // Get attributes from old frame if none are provided.
    //             let attribs = new_attribs.unwrap_or_else(|| entry.get_attributes());
    //             entry.set_attributes(PageAttributes::empty(), AttributeModify::Set);

    //             (unsafe { entry.take_frame_index() }, attribs)
    //         });

    //         maybe_new_pte_frame_index_attribs
    //             .map(|(new_pte_frame_index, new_pte_attribs)| {
    //                 // Create the new page table entry with the old entry's data.
    //                 let entry = map_write.get_page_entry_create(map_to, frame_manager);
    //                 entry.set_frame_index(new_pte_frame_index);
    //                 entry.set_attributes(new_pte_attribs, AttributeModify::Set);

    //                 // Invalidate both old and new pages in TLB.
    //                 #[cfg(target_arch = "x86_64")]
    //                 {
    //                     use crate::arch::x64::instructions::tlb::invlpg;
    //                     invlpg(map_to);
    //                     invlpg(unmap_from);
    //                 }
    //             })
    //             .ok_or(MapError::NotMapped)
    //     })
    // }

    #[inline]
    pub fn auto_map(&self, page: &Page, attribs: PageAttributes, frame_manager: &'static FrameManager<'_>) {
        self.map(page, frame_manager.lock_next().unwrap(), false, attribs, frame_manager).unwrap();
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: &Page) -> bool {
        self.with_root_table(|root_table| root_table.with_entry(&page, |entry| entry.is_present())).unwrap_or(false)
    }

    #[inline]
    pub fn is_mapped_to(&self, page: &Page, frame_index: usize) -> bool {
        self.with_root_table(|root_table| root_table.with_entry(&page, |entry| entry.get_frame_index() == frame_index))
            .unwrap_or(false)
    }

    #[inline]
    pub fn get_mapped_to(&self, page: &Page) -> Option<usize> {
        self.with_root_table(|root_table| root_table.with_entry(&page, |entry| entry.get_frame_index()))
    }

    /* STATE CHANGING */

    #[inline]
    pub fn get_page_attributes(&self, page: &Page) -> Option<PageAttributes> {
        self.with_root_table(|root_table| root_table.with_entry(&page, |entry| entry.get_attributes()))
    }

    pub unsafe fn set_page_attributes(
        &self,
        page: &Page,
        attributes: PageAttributes,
        modify_mode: AttributeModify,
    ) -> Result<(), PagingError> {
        self.with_root_table_mut(|mut root_table| {
            root_table.with_entry_mut(&page, |entry| {
                entry.set_attributes(attributes, modify_mode);

                #[cfg(target_arch = "x86_64")]
                crate::arch::x64::instructions::tlb::invlpg(&page);

                Ok(())
            })
        })
        .unwrap_or(Err(PagingError::NotMapped))
    }

    #[inline]
    pub fn physical_mapped_page(&self) -> Page {
        interrupts::without(|| self.0.read().phys_mapped_page)
    }

    #[inline]
    pub fn read_vmem_register(&self) -> Option<VmemRegister> {
        interrupts::without(|| {
            let vmap = self.0.read();

            #[cfg(target_arch = "x86_64")]
            {
                Some(VmemRegister(
                    Address::<Physical>::new((vmap.root_frame_index * 0x1000) as u64)?,
                    crate::arch::x64::registers::control::CR3Flags::empty(),
                ))
            }
        })
    }

    #[inline]
    pub unsafe fn commit_vmem_register(&self) -> Result<(), PagingError> {
        interrupts::without(|| {
            let vmap = self.0.write();

            let Some(root_frame_index) = Address::<Physical>::new((vmap.root_frame_index * 0x1000) as u64)
                else { return Err(PagingError::InvalidRootFrame) };

            #[cfg(target_arch = "x86_64")]
            crate::arch::x64::registers::control::CR3::write(
                root_frame_index,
                crate::arch::x64::registers::control::CR3Flags::empty(),
            );

            Ok(())
        })
    }
}
