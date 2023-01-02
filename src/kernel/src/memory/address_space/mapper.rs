use core::ptr::NonNull;

use crate::{
    interrupts,
    memory::{AttributeModify, Depth, PageAttributes, PageTable, PageTableEntry, PagingError, PagingRegister, PMM},
};
use lzstd::{
    mem::{Mut, Ref},
    Address, Frame, Page,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapperError {
    NotMapped,
    AlreadyMapped,
    AllocError,
    InvalidRootFrame,
    UnalignedPageAddress,
    PagingError(crate::memory::PagingError),
}

pub struct Mapper {
    depth: usize,
    root_address: Address,
    hhdm_ptr: NonNull<u8>,
    entry: PageTableEntry,
}

// Safety: Type has no thread-local references.
unsafe impl Send for Mapper {}

impl Mapper {
    /// Attempts to construct a new page manager. Returns `None` if the provided page table depth is not supported.
    /// ### Safety
    ///
    /// Refer to `VirtualMapper::new()`.
    pub unsafe fn new(depth: usize, hhdm_ptr: NonNull<u8>, vmem_register_copy: Option<PagingRegister>) -> Option<Self> {
        const VALID_DEPTHS: core::ops::RangeInclusive<usize> = 3..=5;

        if VALID_DEPTHS.contains(&depth)
            && let Ok(root_frame) = PMM.next_frame()
            && let Some(root_mapped_address) = Address::<Virtual>::new((hhdm_ptr.addr().get() as u64) + root_frame.as_u64())
        {
            match vmem_register_copy {
                Some(vmem_register_copy) if let Some(copy_mapped_address) =   Address::<Virtual>::new(hhdm_ptr.as_u64() + vmem_register_copy.0.as_u64()) =>{
                    core::ptr::copy_nonoverlapping(copy_mapped_address.as_ptr::<u8>(), root_mapped_address.as_mut_ptr::<u8>(), 0x1000);
                },
                _ => core::ptr::write_bytes(root_mapped_address.as_mut_ptr::<u8>(), 0, 0x1000),
            }

            Some(Self{ depth, root_address: root_frame, hhdm_ptr, entry: PageTableEntry::new(root_frame, PageAttributes::PRESENT) })
        } else {
            None
        }
    }

    /// # Safety
    ///
    /// * The provided higher-half direct mapped address must be valid, and;
    /// * There are no synchronization checks done to ensure this instance isn't concurrent with another context.
    pub unsafe fn from_current(hhdm_ptr: NonNull<u8>) -> Self {
        let root_frame = PagingRegister::read().frame();
        let root_table_entry = PageTableEntry::new(root_frame, PageAttributes::PRESENT);

        Self {
            // TODO fix this for rv64 Sv39
            depth: if crate::memory::supports_5_level_paging() && crate::memory::is_5_level_paged() { 5 } else { 4 },
            root_address: root_frame,
            hhdm_ptr,
            entry: root_table_entry,
        }
    }

    fn with_root_table<T>(&self, func: impl FnOnce(PageTable<Ref>) -> T) -> T {
        interrupts::without(|| {
            // TODO try to find alternative to unwrapping here
            // ### Safety: `VirtualMapper` already requires that the physical mapping page is valid, so it can be safely passed to the page table.
            func(unsafe { PageTable::<Ref>::new(self.depth, self.hhdm_ptr, &self.entry).unwrap_unchecked() })
        })
    }

    fn with_root_table_mut<T>(&mut self, func: impl FnOnce(PageTable<Mut>) -> T) -> T {
        interrupts::without(|| {
            // ### Safety: `VirtualMapper` already requires that the physical mapping page is valid, so it can be safely passed to the page table.
            func(unsafe { PageTable::<Mut>::new(self.depth, self.hhdm_ptr, &mut self.entry).unwrap_unchecked() })
        })
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the specified frame index.
    pub fn map(
        &mut self,
        page: Page,
        depth: Depth,
        frame: Frame,
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
            root_table.with_entry_create(page, depth, |entry| {
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
    pub unsafe fn unmap(&mut self, page: Page, depth: Option<Depth>, free_frame: bool) -> Result<(), PagingError> {
        self.with_root_table_mut(|mut root_table| {
            root_table.with_entry_mut(page, depth, |entry| {
                entry.map(|entry| {
                    // ### Safety: We've got an explicit directive from the caller to unmap this page, so the caller must ensure that's a valid operation.
                    unsafe { entry.set_attributes(PageAttributes::PRESENT, AttributeModify::Remove) };

                    let frame = entry.get_frame();
                    // ### Safety: See above.
                    unsafe { entry.set_frame(Frame::zero()) };

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

    pub fn auto_map(&mut self, page: Page, attributes: PageAttributes) -> Result<(), MapperError> {
        match PMM.next_frame() {
            Ok(frame) => self.map(page, Depth::min(), frame, !attributes.contains(PageAttributes::DEMAND), attributes),
            Err(_) => Err(MapperError::AllocError),
        }
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: Page, depth: Option<Depth>) -> bool {
        self.with_root_table(|root_table| root_table.with_entry(page, depth, |entry| entry.is_ok()))
    }

    pub fn is_mapped_to(&self, page: Page, frame: Frame) -> bool {
        self.with_root_table(|root_table| {
            root_table.with_entry(page, None, |entry| entry.map(|entry| entry.get_frame() == frame).unwrap_or(false))
        })
    }

    pub fn get_mapped_to(&self, page: Page) -> Option<Frame> {
        self.with_root_table(|root_table| {
            root_table.with_entry(page, None, |entry| entry.ok().map(|entry| entry.get_frame()))
        })
    }

    /* STATE CHANGING */

    pub fn get_page_attributes(&self, page: Page) -> Option<PageAttributes> {
        self.with_root_table(|root_table| {
            root_table.with_entry(page, None, |entry| match entry {
                Ok(entry) => Some(entry.get_attributes()),
                Err(_) => None,
            })
        })
    }

    pub unsafe fn set_page_attributes(
        &mut self,
        page: Page,
        depth: Option<Depth>,
        attributes: PageAttributes,
        modify_mode: AttributeModify,
    ) -> Result<(), MapperError> {
        self.with_root_table_mut(|mut root_table| {
            root_table.with_entry_mut(page, depth, |entry| match entry {
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

    pub fn physical_mapped_address(&self) -> NonNull<u8> {
        interrupts::without(|| self.hhdm_ptr)
    }

    /// ### Safety
    ///
    /// Caller must ensure that committing this mapper's parameters to the virtual memory register will
    ///         not result in undefined behaviour.
    pub unsafe fn commit_vmem_register(&self) -> Result<(), MapperError> {
        interrupts::without(|| {
            #[cfg(target_arch = "x86_64")]
            crate::arch::x64::registers::control::CR3::write(
                self.root_address,
                crate::arch::x64::registers::control::CR3Flags::empty(),
            );

            Ok(())
        })
    }
}
