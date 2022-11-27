use crate::{
    interrupts,
    memory::{AttributeModify, PageAttributes, PageTable, PageTableEntry, PagingRegister, PMM},
};
use lzstd::{
    mem::{Mut, Ref},
    Address, Frame, Page, Virtual,
};
use spin::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapperError {
    NotMapped,
    AlreadyMapped,
    AllocError,
    InvalidRootFrame,
    UnalignedPageAddress,
    PagingError(crate::memory::PagingError),
}

struct Data {
    depth: usize,
    root_frame: Address<Frame>,
    hhdm_address: Address<Virtual>,
    entry: PageTableEntry,
}

/// FIXME: This needs to not use internal locking (i.e. not `Sync`).
pub struct Mapper(RwLock<Data>);

// ### Safety: Type is designed to be thread-agnostic internally.
unsafe impl Sync for Mapper {}

impl Mapper {
    /// Attempts to construct a new page manager. Returns `None` if the provided page table depth is not supported.
    /// ### Safety
    ///
    /// Refer to `VirtualMapper::new()`.
    pub unsafe fn new(
        depth: usize,
        phys_mapped_address: Address<Virtual>,
        vmem_register_copy: Option<PagingRegister>,
    ) -> Option<Self> {
        const VALID_DEPTHS: core::ops::RangeInclusive<usize> = 3..=5;

        if VALID_DEPTHS.contains(&depth)
            && let Ok(root_frame) = PMM.next_frame()
            && let Some(root_mapped_address) = Address::<Virtual>::new(phys_mapped_address.as_u64() + root_frame.as_u64())
        {
            match vmem_register_copy {
                Some(vmem_register_copy) if let Some(copy_mapped_address) =   Address::<Virtual>::new(phys_mapped_address.as_u64() + vmem_register_copy.0.as_u64()) =>{
                    core::ptr::copy_nonoverlapping(copy_mapped_address.as_ptr::<u8>(), root_mapped_address.as_mut_ptr::<u8>(), 0x1000);
                },
                _ => core::ptr::write_bytes(root_mapped_address.as_mut_ptr::<u8>(), 0, 0x1000),
            }

            Some(Self(RwLock::new(Data { depth, root_frame, hhdm_address: phys_mapped_address, entry: PageTableEntry::new(root_frame, PageAttributes::PRESENT) })))
        } else {
            None
        }
    }

    pub unsafe fn from_current(hhdm_address: Address<Virtual>) -> Self {
        let root_frame = PagingRegister::read().frame();
        let root_table_entry = PageTableEntry::new(root_frame, PageAttributes::PRESENT);

        Self(RwLock::new(Data {
            // TODO fix this for rv64 Sv39
            depth: if crate::memory::supports_5_level_paging() && crate::memory::is_5_level_paged() { 5 } else { 4 },
            root_frame,
            hhdm_address,
            entry: root_table_entry,
        }))
    }

    fn with_root_table<T>(&self, func: impl FnOnce(PageTable<Ref>) -> T) -> T {
        interrupts::without(|| {
            let data = self.0.read();
            // TODO try to find alternative to unwrapping here
            // ### Safety: `VirtualMapper` already requires that the physical mapping page is valid, so it can be safely passed to the page table.
            func(unsafe { PageTable::<Ref>::new(data.depth, data.hhdm_address, &data.entry).unwrap() })
        })
    }

    fn with_root_table_mut<T>(&self, func: impl FnOnce(PageTable<Mut>) -> T) -> T {
        interrupts::without(|| {
            let mut data = self.0.write();
            // ### Safety: `VirtualMapper` already requires that the physical mapping page is valid, so it can be safely passed to the page table.
            func(unsafe { PageTable::<Mut>::new(data.depth, data.hhdm_address, &mut data.entry).unwrap() })
        })
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the specified frame index.
    pub fn map(
        &self,
        page: Address<Page>,
        frame: Address<Frame>,
        lock_frames: bool,
        mut attributes: PageAttributes,
    ) -> Result<(), MapperError> {
        let result = self.with_root_table_mut(|mut root_table| {
            let Some(page_align) = page.align() else { return Err(MapperError::UnalignedPageAddress) };
            // If the acquisition of the frame fails, return the error.
            if lock_frames && PMM.lock_frames(frame, page_align.as_usize() / 0x1000).is_err() {
                return Err(MapperError::AllocError);
            }

            // If acquisition of the frame is successful, attempt to map the page to the frame index.
            root_table.with_entry_create(page, |entry| {
                match entry {
                    Ok(entry) => {
                        *entry = PageTableEntry::new(frame, {
                            // Make sure the `HUGE` bit is automatically set for huge pages.
                            if page.depth().unwrap_or(1) > 1 {
                                attributes.insert(PageAttributes::HUGE);
                            }

                            attributes
                        });

                        #[cfg(target_arch = "x86_64")]
                        crate::arch::x64::instructions::tlb::invlpg(page);

                        Ok(())
                    }

                    Err(err) => Err(MapperError::PagingError(err)),
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
    /// ### Safety
    ///
    /// Caller must ensure calling this function does not cause memory corruption.
    pub unsafe fn unmap(&self, page: Address<Page>, free_frame: bool) -> Result<(), super::PagingError> {
        self.with_root_table_mut(|mut root_table| {
            root_table.with_entry_mut(page, |entry| {
                entry.map(|entry| {
                    // ### Safety: We've got an explicit directive from the caller to unmap this page, so the caller must ensure that's a valid operation.
                    unsafe { entry.set_attributes(PageAttributes::PRESENT, AttributeModify::Remove) };

                    let frame = entry.get_frame();
                    // ### Safety: See above.
                    unsafe { entry.set_frame(Address::<Frame>::zero()) };

                    if free_frame {
                        PMM.free_frame(frame).unwrap();
                    }

                    // Invalidate the page in the TLB.
                    #[cfg(target_arch = "x86_64")]
                    crate::arch::x64::instructions::tlb::invlpg(page);
                })
            })
        })
    }

    pub fn auto_map(&self, page: Address<Page>, attributes: PageAttributes) -> Result<(), MapperError> {
        match PMM.next_frame() {
            Ok(frame) => self.map(page, frame, !attributes.contains(PageAttributes::DEMAND), attributes),
            Err(_) => Err(MapperError::AllocError),
        }
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: Address<Page>) -> bool {
        self.with_root_table(|root_table| root_table.with_entry(page, |entry| entry.is_ok()))
    }

    pub fn is_mapped_to(&self, page: Address<Page>, frame: Address<Frame>) -> bool {
        self.with_root_table(|root_table| {
            root_table.with_entry(page, |entry| entry.map(|entry| entry.get_frame() == frame).unwrap_or(false))
        })
    }

    pub fn get_mapped_to(&self, page: Address<Page>) -> Option<Address<Frame>> {
        self.with_root_table(|root_table| {
            root_table.with_entry(page, |entry| entry.ok().map(|entry| entry.get_frame()))
        })
    }

    pub fn map_if_not_mapped(
        &self,
        page: Address<Page>,
        frame_and_lock: Option<(Address<Frame>, bool)>,
        attributes: PageAttributes,
    ) -> Result<(), MapperError> {
        match frame_and_lock {
            Some((frame, lock_frame)) if !self.is_mapped_to(page, frame) => {
                self.map(page, frame, lock_frame, attributes)
            }

            None if !self.is_mapped(page) => self.auto_map(page, attributes),

            _ => Ok(()),
        }
    }

    /* STATE CHANGING */

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
    ) -> Result<(), MapperError> {
        self.with_root_table_mut(|mut root_table| {
            root_table.with_entry_mut(page, |entry| match entry {
                Ok(entry) => {
                    entry.set_attributes(attributes, modify_mode);

                    #[cfg(target_arch = "x86_64")]
                    crate::arch::x64::instructions::tlb::invlpg(page);

                    Ok(())
                }

                Err(err) => Err(MapperError::PagingError(err)),
            })
        })
    }

    pub fn physical_mapped_address(&self) -> Address<Virtual> {
        interrupts::without(|| self.0.read().hhdm_address)
    }

    pub fn read_vmem_register(&self) -> Option<PagingRegister> {
        interrupts::without(|| {
            let vmap = self.0.read();

            #[cfg(target_arch = "x86_64")]
            {
                Some(PagingRegister(vmap.root_frame, crate::arch::x64::registers::control::CR3Flags::empty()))
            }
        })
    }

    /// ### Safety
    ///
    /// Caller must ensure that committing this mapper's parameters to the virtual memory register will
    ///         not result in undefined behaviour.
    pub unsafe fn commit_vmem_register(&self) -> Result<(), MapperError> {
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
