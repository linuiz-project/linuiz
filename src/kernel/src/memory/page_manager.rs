use crate::{
    interrupts,
    memory::{AttributeModify, Mut, PageAttributes, PageTable, PageTableEntry, Ref},
};
use libkernel::{Address, Frame, Page, Virtual};
use spin::RwLock;

use super::VmemRegister;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageManagerError {
    NotMapped,
    AlreadyMapped,
    AllocError,
    InvalidRootFrame,
    PagingError(crate::memory::PagingError),
}

struct PageManagerData {
    depth: usize,
    root_frame: Address<Frame>,
    phys_mapped_address: Address<Virtual>,
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
        phys_mapped_address: Address<Virtual>,
        vmem_register_copy: Option<VmemRegister>,
    ) -> Option<Self> {
        const VALID_DEPTHS: core::ops::RangeInclusive<usize> = 3..=5;

        let global_allocator = crate::memory::get_global_allocator();

        if VALID_DEPTHS.contains(&depth)
            && let Ok(root_frame) = global_allocator.lock_next()
            && let Some(root_mapped_address) = Address::<Virtual>::new(phys_mapped_address.as_u64() + root_frame.as_u64())
        {
            match vmem_register_copy {
                Some(vmem_register_copy) if let Some(copy_mapped_address) =   Address::<Virtual>::new(phys_mapped_address.as_u64() + vmem_register_copy.0.as_u64()) =>{
                    core::ptr::copy_nonoverlapping(copy_mapped_address.as_ptr::<u8>(), root_mapped_address.as_mut_ptr::<u8>(), 0x1000);
                },
                _ => core::ptr::write_bytes(root_mapped_address.as_mut_ptr::<u8>(), 0, 0x1000),
            }

            Some(Self(RwLock::new(PageManagerData { depth, root_frame, phys_mapped_address, entry: PageTableEntry::new(root_frame, PageAttributes::PRESENT) })))
        } else {
            None
        }
    }

    pub unsafe fn from_current(phys_mapped_address: Address<Virtual>) -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let root_frame = crate::arch::x64::registers::control::CR3::read().0;
            let root_table_entry = PageTableEntry::new(root_frame, PageAttributes::PRESENT);
            if let Some(ext_feature_info) = crate::arch::x64::cpu::cpuid::EXT_FEATURE_INFO.as_ref()
                && ext_feature_info.has_la57()
                && crate::arch::x64::registers::control::CR4::read().contains(crate::arch::x64::registers::control::CR4Flags::LA57)
            {
                Self(RwLock::new(PageManagerData { depth: 5, root_frame, phys_mapped_address, entry: root_table_entry }))
            } else {
                Self(RwLock::new(PageManagerData { depth: 4,  root_frame, phys_mapped_address, entry: root_table_entry }))
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
            // TODO try to find alternative to unwrapping here
            // SAFETY: `PageManager` already requires that the physical mapping page is valid, so it can be safely passed to the page table.
            func(unsafe { PageTable::<Ref>::new(data.depth, data.phys_mapped_address, &data.entry).unwrap() })
        })
    }

    fn with_root_table_mut<T>(&self, func: impl FnOnce(PageTable<Mut>) -> T) -> T {
        interrupts::without(|| {
            let mut data = self.0.write();
            // SAFETY: `PageManager` already requires that the physical mapping page is valid, so it can be safely passed to the page table.
            func(unsafe { PageTable::<Mut>::new(data.depth, data.phys_mapped_address, &mut data.entry).unwrap() })
        })
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the specified frame index.
    pub fn map(
        &self,
        page: Address<Page>,
        frame: Address<Frame>,
        attributes: PageAttributes,
    ) -> Result<(), PageManagerError> {
        let result = self.with_root_table_mut(|mut root_table| {
            let frame_count = (page.align() as usize) / 0x1000;
            // If the acquisition of the frame fails, return the error.
            if crate::memory::get_global_allocator().borrow_many(frame, frame_count as usize).is_err() {
                return Err(PageManagerError::AllocError);
            }

            // If acquisition of the frame is successful, attempt to map the page to the frame index.
            root_table.with_entry_create(page, |entry| {
                match entry {
                    Ok(entry) => {
                        // SAFETY: We've got an explicit directive from the caller to map these pages, and we've checked the condition of the
                        //         pages and entries, so if this isn't safe it's on the caller.
                        unsafe {
                            entry.set_frame(frame);
                            entry.set_attributes(attributes, AttributeModify::Set);
                        }

                        #[cfg(target_arch = "x86_64")]
                        crate::arch::x64::instructions::tlb::invlpg(page);

                        Ok(())
                    }

                    Err(err) => Err(PageManagerError::PagingError(err)),
                }
            })
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
    /// SAFETY: Caller must ensure calling this function does not cause memory corruption.
    pub unsafe fn unmap(&self, page: Address<Page>) -> Result<(), PageManagerError> {
        self.with_root_table_mut(|mut root_table| {
            root_table.with_entry_mut(page, |entry| {
                match entry {
                    Ok(entry) => {
                        // SAFETY: We've got an explicit directive from the caller to unmap this page, so the caller must ensure that's a valid operation.
                        unsafe { entry.set_attributes(PageAttributes::PRESENT, AttributeModify::Remove) };

                        // Handle frame permissions to keep them updated.
                        if free_frame {
                            let frame_index = entry.get_frame();

                            // SAFETY: See above.
                            unsafe {
                                entry.set_frame(Address::<Frame>::zero());
                                frame_manager.free(frame_index).unwrap();
                            }
                        }

                        // Invalidate the page in the TLB.
                        #[cfg(target_arch = "x86_64")]
                        crate::arch::x64::instructions::tlb::invlpg(page);

                        Ok(())
                    }

                    Err(err) => Err(PageManagerError::PagingError(err)),
                }
            })
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
    pub fn auto_map(&self, page: Address<Page>, attributes: PageAttributes, frame_manager: &'static FrameManager<'_>) {
        self.map(
            page,
            frame_manager.lock_next_many((page.align() as usize) / 0x1000).unwrap(),
            false,
            attributes,
            frame_manager,
        )
        .unwrap();
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: Address<Page>) -> bool {
        self.with_root_table(|root_table| root_table.with_entry(page, |entry| entry.is_ok()))
    }

    #[inline]
    pub fn is_mapped_to(&self, page: Address<Page>, frame: Address<Frame>) -> bool {
        self.with_root_table(|root_table| {
            root_table.with_entry(page, |entry| entry.map(|entry| entry.get_frame() == frame).unwrap_or(false))
        })
    }

    #[inline]
    pub fn get_mapped_to(&self, page: Address<Page>) -> Option<Address<Frame>> {
        self.with_root_table(|root_table| {
            root_table.with_entry(page, |entry| entry.ok().map(|entry| entry.get_frame()))
        })
    }

    /* STATE CHANGING */

    #[inline]
    pub fn get_page_attributes(&self, page: Address<Page>) -> Option<PageAttributes> {
        self.with_root_table(|root_table| {
            root_table.with_entry(page, |entry| match entry {
                Ok(entry) => Some(entry.get_attributes()),
                Err(_) => None,
            })
        })
    }

    pub unsafe fn set_page_attributes(
        &self,
        page: Address<Page>,
        attributes: PageAttributes,
        modify_mode: AttributeModify,
    ) -> Result<(), PageManagerError> {
        self.with_root_table_mut(|mut root_table| {
            root_table.with_entry_mut(page, |entry| match entry {
                Ok(entry) => {
                    entry.set_attributes(attributes, modify_mode);

                    #[cfg(target_arch = "x86_64")]
                    crate::arch::x64::instructions::tlb::invlpg(page);

                    Ok(())
                }

                Err(err) => Err(PageManagerError::PagingError(err)),
            })
        })
    }

    #[inline]
    pub fn physical_mapped_address(&self) -> Address<Virtual> {
        interrupts::without(|| self.0.read().phys_mapped_address)
    }

    #[inline]
    pub fn read_vmem_register(&self) -> Option<VmemRegister> {
        interrupts::without(|| {
            let vmap = self.0.read();

            #[cfg(target_arch = "x86_64")]
            {
                Some(VmemRegister(vmap.root_frame, crate::arch::x64::registers::control::CR3Flags::empty()))
            }
        })
    }

    #[inline]
    pub unsafe fn commit_vmem_register(&self) -> Result<(), PageManagerError> {
        interrupts::without(|| {
            let vmap = self.0.write();

            #[cfg(target_arch = "x86_64")]
            crate::arch::x64::registers::control::CR3::write(
                vmap.root_frame,
                crate::arch::x64::registers::control::CR3Flags::empty(),
            );

            Ok(())
        })
    }
}
