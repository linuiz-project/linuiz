use crate::memory::{AttributeModify, PageTable, PageTableEntry};
use libarch::{
    interrupts,
    memory::{PageAttributes, VmemRegister},
};
use libcommon::{
    memory::{Mut, Ref},
    Address, Frame, Page, Virtual,
};
use spin::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualMapperError {
    NotMapped,
    AlreadyMapped,
    AllocError,
    InvalidRootFrame,
    UnalignedPageAddress,
    PagingError(crate::memory::PagingError),
}

struct VirtualMapperData {
    depth: usize,
    root_frame: Address<Frame>,
    phys_mapped_address: Address<Virtual>,
    entry: PageTableEntry,
}

pub struct VirtualMapper(RwLock<VirtualMapperData>);

// SAFETY: Type is designed to be thread-agnostic internally.
unsafe impl Sync for VirtualMapper {}

impl VirtualMapper {
    /// Attempts to construct a new page manager. Returns `None` if the provided page table depth is not supported.
    /// SAFETY: Refer to `VirtualMapper::new()`.
    pub unsafe fn new(
        depth: usize,
        phys_mapped_address: Address<Virtual>,
        vmem_register_copy: Option<VmemRegister>,
    ) -> Option<Self> {
        const VALID_DEPTHS: core::ops::RangeInclusive<usize> = 3..=5;

        if VALID_DEPTHS.contains(&depth)
            && let Ok(root_frame) = libcommon::memory::get_global_allocator().lock_next()
            && let Some(root_mapped_address) = Address::<Virtual>::new(phys_mapped_address.as_u64() + root_frame.as_u64())
        {
            match vmem_register_copy {
                Some(vmem_register_copy) if let Some(copy_mapped_address) =   Address::<Virtual>::new(phys_mapped_address.as_u64() + vmem_register_copy.0.as_u64()) =>{
                    core::ptr::copy_nonoverlapping(copy_mapped_address.as_ptr::<u8>(), root_mapped_address.as_mut_ptr::<u8>(), 0x1000);
                },
                _ => core::ptr::write_bytes(root_mapped_address.as_mut_ptr::<u8>(), 0, 0x1000),
            }

            Some(Self(RwLock::new(VirtualMapperData { depth, root_frame, phys_mapped_address, entry: PageTableEntry::new(root_frame, PageAttributes::PRESENT) })))
        } else {
            None
        }
    }

    pub unsafe fn from_current(phys_mapped_address: Address<Virtual>) -> Self {
        let root_frame = libarch::memory::VmemRegister::read().frame();
        let root_table_entry = PageTableEntry::new(root_frame, PageAttributes::PRESENT);

        Self(RwLock::new(VirtualMapperData {
            // TODO fix this for rv64 Sv39
            depth: if libarch::memory::supports_5_level_paging() && libarch::memory::is_5_level_paged() {
                5
            } else {
                4
            },
            root_frame,
            phys_mapped_address,
            entry: root_table_entry,
        }))
    }

    fn with_root_table<T>(&self, func: impl FnOnce(PageTable<Ref>) -> T) -> T {
        interrupts::without(|| {
            let data = self.0.read();
            // TODO try to find alternative to unwrapping here
            // SAFETY: `VirtualMapper` already requires that the physical mapping page is valid, so it can be safely passed to the page table.
            func(unsafe { PageTable::<Ref>::new(data.depth, data.phys_mapped_address, &data.entry).unwrap() })
        })
    }

    fn with_root_table_mut<T>(&self, func: impl FnOnce(PageTable<Mut>) -> T) -> T {
        interrupts::without(|| {
            let mut data = self.0.write();
            // SAFETY: `VirtualMapper` already requires that the physical mapping page is valid, so it can be safely passed to the page table.
            func(unsafe { PageTable::<Mut>::new(data.depth, data.phys_mapped_address, &mut data.entry).unwrap() })
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
    ) -> Result<(), VirtualMapperError> {
        let result = self.with_root_table_mut(|mut root_table| {
            let Some(page_align) = page.align() else { return Err(VirtualMapperError::UnalignedPageAddress) };
            // If the acquisition of the frame fails, return the error.
            if lock_frames
                && libcommon::memory::get_global_allocator().lock_many(frame, page_align.as_usize() / 0x1000).is_err()
            {
                return Err(VirtualMapperError::AllocError);
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
                        libarch::x64::instructions::tlb::invlpg(page);

                        Ok(())
                    }

                    Err(err) => Err(VirtualMapperError::PagingError(err)),
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
    pub unsafe fn unmap(&self, page: Address<Page>) -> Result<(), VirtualMapperError> {
        self.with_root_table_mut(|mut root_table| {
            root_table.with_entry_mut(page, |entry| {
                match entry {
                    Ok(entry) => {
                        // SAFETY: We've got an explicit directive from the caller to unmap this page, so the caller must ensure that's a valid operation.
                        unsafe { entry.set_attributes(PageAttributes::PRESENT, AttributeModify::Remove) };

                        let frame = entry.get_frame();
                        // SAFETY: See above.
                        unsafe { entry.set_frame(Address::<Frame>::zero()) };
                        libcommon::memory::get_global_allocator().free(frame).unwrap();

                        // Invalidate the page in the TLB.
                        #[cfg(target_arch = "x86_64")]
                        libarch::x64::instructions::tlb::invlpg(page);

                        Ok(())
                    }

                    Err(err) => Err(VirtualMapperError::PagingError(err)),
                }
            })
        })
    }

    pub fn auto_map(&self, page: Address<Page>, attributes: PageAttributes) -> Result<(), VirtualMapperError> {
        match libcommon::memory::get_global_allocator().lock_next() {
            Ok(frame) => self.map(page, frame, false, attributes),
            Err(_) => Err(VirtualMapperError::AllocError),
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
    ) -> Result<(), VirtualMapperError> {
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
    ) -> Result<(), VirtualMapperError> {
        self.with_root_table_mut(|mut root_table| {
            root_table.with_entry_mut(page, |entry| match entry {
                Ok(entry) => {
                    entry.set_attributes(attributes, modify_mode);

                    #[cfg(target_arch = "x86_64")]
                    libarch::x64::instructions::tlb::invlpg(page);

                    Ok(())
                }

                Err(err) => Err(VirtualMapperError::PagingError(err)),
            })
        })
    }

    pub fn physical_mapped_address(&self) -> Address<Virtual> {
        interrupts::without(|| self.0.read().phys_mapped_address)
    }

    pub fn read_vmem_register(&self) -> Option<VmemRegister> {
        interrupts::without(|| {
            let vmap = self.0.read();

            #[cfg(target_arch = "x86_64")]
            {
                Some(VmemRegister(vmap.root_frame, libarch::x64::registers::control::CR3Flags::empty()))
            }
        })
    }

    pub unsafe fn commit_vmem_register(&self) -> Result<(), VirtualMapperError> {
        interrupts::without(|| {
            let vmap = self.0.write();

            #[cfg(target_arch = "x86_64")]
            libarch::x64::registers::control::CR3::write(
                vmap.root_frame,
                libarch::x64::registers::control::CR3Flags::empty(),
            );

            Ok(())
        })
    }
}
