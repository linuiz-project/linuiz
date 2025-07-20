use crate::mem::{
    HigherHalfDirectMap, Permissions, paging::page_table::PageTable, pmm::PhysicalMemoryManager,
};
use bit_field::BitField;
use core::{alloc::AllocError, ptr::NonNull};
use libsys::{Address, Frame};

#[derive(FromZeros, Clone, Copy)]
pub struct Entry(usize);

impl Entry {
    #[cfg(target_arch = "x86_64")]
    const PRESENT_BIT_INDEX: usize = 0;
    #[cfg(target_arch = "x86_64")]
    const WRITEABLE_BIT_INDEX: usize = 1;
    #[cfg(target_arch = "x86_64")]
    const USER_BIT_INDEX: usize = 2;
    #[cfg(target_arch = "x86_64")]
    const HUGE_PAGE_BIT_INDEX: usize = 7;
    #[cfg(target_arch = "x86_64")]
    const NO_EXECUTE_BIT_INDEX: usize = 63;

    #[cfg(target_arch = "riscv64")]
    const VALID_BIT_INDEX: usize = 0;
    #[cfg(target_arch = "riscv64")]
    const READABLE_BIT_INDEX: usize = 1;
    #[cfg(target_arch = "riscv64")]
    const WRITEABLE_BIT_INDEX: usize = 2;
    #[cfg(target_arch = "riscv64")]
    const EXECUTABLE_BIT_INDEX: usize = 3;
    #[cfg(target_arch = "riscv64")]
    const USER_BIT_INDEX: usize = 4;
    #[cfg(target_arch = "riscv64")]
    const GLOBAL_BIT_INDEX: usize = 5;

    fn get_frame_address_range() -> core::ops::Range<usize> {
        cfg_select! {
            target_arch = "x86_64" => {
                debug_assert!(
                    crate::arch::x86_64::cpuid::feature_info()
                        .is_some_and(raw_cpuid::FeatureInfo::has_pae)
                        && crate::arch::x86_64::registers::control::CR4::read()
                            .contains(crate::arch::x86_64::registers::control::CR4Flags::PAE)
                );

                12..51
            }

            _ => { todo!() }
        }
    }

    pub unsafe fn clear(&mut self) {
        self.0 = 0;
    }

    /// Gets the frame index of the page table entry.
    pub fn get_frame(self) -> Address<Frame> {
        let frame_index = self.0.get_bits(Self::get_frame_address_range());
        let frame_address = Address::from_index(frame_index);

        debug_assert!(frame_address.is_some());

        // Safety: Page table pointers are guaranteed to have valid physical addresses.
        unsafe { frame_address.unwrap_unchecked() }
    }

    /// Sets the entry's frame index.
    pub unsafe fn set_frame(&mut self, frame: Address<Frame>) {
        self.0
            .set_bits(Self::get_frame_address_range(), frame.index());
    }

    pub fn is_enabled(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.get_bit(Self::PRESENT_BIT_INDEX)
            }

            target_arch = "riscv64" => {
                self.0.get_bit(Self::VALID_BIT_INDEX)
            }

            _ => { todo!() }
        }
    }

    pub unsafe fn set_enabled(&mut self, enabled: bool) {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.set_bit(Self::PRESENT_BIT_INDEX, enabled);
            }

            target_arch = "riscv64" => {
                self.0.set_bit(Self::VALID_BIT_INDEX, enabled);
            }

            _ => { todo!() }
        }
    }

    #[cfg(target_arch = "riscv64")]
    pub fn is_global(&self) -> bool {
        cfg_select! {
            target_arch = "riscv64" => { self.0.get_bit(Self::GLOBAL_BIT_INDEX) }

            _ => { todo!() }
        }
    }

    #[cfg(target_arch = "riscv64")]
    #[allow(unused_variables)]
    pub unsafe fn set_global(&mut self, global: bool) {
        cfg_select! {
            target_arch = "riscv64" => {
                self.0.set_bit(Self::GLOBAL_BIT_INDEX, global);
            }

            _ => { todo!() }
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn is_huge(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.get_bit(Self::HUGE_PAGE_BIT_INDEX)
            }

            _ => { todo!() }
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub unsafe fn set_huge(&mut self, huge_page: bool) {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.set_bit(Self::HUGE_PAGE_BIT_INDEX, huge_page);
            }

            _ => { todo!() }
        }
    }

    pub fn is_intermediate(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => { !self.is_huge() }

            _ => { todo!() }
        }
    }

    pub fn get_permissions(&self) -> Permissions {
        cfg_select! {
            target_arch = "x86_64" => {
                match (
                    self.0.get_bit(Self::WRITEABLE_BIT_INDEX),
                    self.0.get_bit(Self::NO_EXECUTE_BIT_INDEX),
                ) {
                    (false, true) => Permissions::ReadOnly,
                    (true, true) => Permissions::ReadWrite,
                    (false, false) => Permissions::ReadExecute,

                    _ => unreachable!(),
                }
            }

            _ => { todo!() }
        }
    }

    pub unsafe fn set_permissions(&mut self, permissions: Permissions) {
        cfg_select! {
            target_arch = "x86_64" => {
                match permissions {
                    // All pages in x86_64 are at least read only, so `Permissions::None` is effectively
                    // analogous to that.
                    Permissions::None | Permissions::ReadOnly => {
                        self.0.set_bit(Self::WRITEABLE_BIT_INDEX, false);
                        self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, true);
                    }

                    Permissions::ReadWrite => {
                        self.0.set_bit(Self::WRITEABLE_BIT_INDEX, true);
                        self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, true);
                    }

                    Permissions::ReadExecute => {
                        self.0.set_bit(Self::WRITEABLE_BIT_INDEX, false);
                        self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, false);
                    }
                }
            }

            _ => { todo!() }
        }
    }

    pub fn is_user_accessible(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => { self.0.get_bit(Self::USER_BIT_INDEX) }
            target_arch = "riscv64" => { self.0.get_bit(Self::USER_BIT_INDEX) }

            _ => { todo!() }
        }
    }

    pub unsafe fn set_user_accessible(&mut self, user_accessible: bool) {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.set_bit(Self::USER_BIT_INDEX, user_accessible);
            }

            target_arch = "riscv64" => {
                self.0.set_bit(Self::USER_BIT_INDEX, user_accessible);
            }

            _ => { todo!() }
        }
    }

    /// Automatically populate the pointer with a new frame.
    pub fn populate(&mut self) -> Result<(), AllocError> {
        debug_assert!(!self.is_enabled());

        let frame = PhysicalMemoryManager::next_frame().ok_or(AllocError)?;

        // Safety: Memory was just allocated.
        unsafe {
            crate::mem::zero_frame(frame);
        }

        unsafe {
            self.set_frame(frame);
            self.set_enabled(true);
        }

        Ok(())
    }

    /// Automatically depopulate the pointer and free the frame.
    pub fn depopulate(&mut self) -> Result<(), AllocError> {
        debug_assert!(!self.is_enabled());

        let frame = self.get_frame();

        PhysicalMemoryManager::free_frame(frame);

        unsafe {
            self.set_frame(Address::default());
            self.set_enabled(false);
        }

        Ok(())
    }

    /// Gets the higher-half direct mapped pointer to the page table this pointer refers to, or
    /// `None`.
    fn get_page_table_ptr(&self) -> Option<NonNull<PageTable>> {
        if !self.is_enabled() {
            return None;
        }

        let frame = self.get_frame();
        let page = HigherHalfDirectMap::frame_to_page(frame);

        // This pointer—coming from the higher-half direct map—should never be null.
        let ptr = page.as_ptr().cast::<PageTable>();

        debug_assert!(!ptr.is_null());
        debug_assert!(ptr.is_aligned_to(align_of::<PageTable>()));

        let ptr = unsafe { NonNull::new_unchecked(ptr) };

        Some(ptr)
    }

    /// Gets a shared reference the page table this pointer refers to, or `None`.
    pub(in crate::mem::paging) fn page_table(&self) -> Option<&PageTable> {
        let ptr = self.get_page_table_ptr()?;

        // Safety:
        //  - Pointer is required to be non-null if it's enabled.
        //  - Pointer is naturally aligned due to the layout of `Pointer`.
        //  - Caller is required to ensure the page table is zeroed or initialized.
        //  - `&self` aliases as a shared reference.
        let page_table = unsafe { ptr.as_ref() };

        Some(page_table)
    }

    /// Gets an exclusive reference the page table this pointer refers to, or `None`.
    pub(in crate::mem::paging) fn page_table_mut(&mut self) -> Option<&mut PageTable> {
        let mut ptr = self.get_page_table_ptr()?;

        // Safety:
        //  - Pointer is required to be non-null if it's enabled.
        //  - Pointer is naturally aligned due to the layout of `Pointer`.
        //  - Caller is required to ensure the page table is zeroed or initialized.
        //  - `&self` aliases as a shared reference.
        let page_table = unsafe { ptr.as_mut() };

        Some(page_table)
    }
}

impl core::fmt::Debug for Entry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = formatter.debug_struct("Page Table Pointer");

        debug_struct
            .field("Physical Address", &self.get_frame())
            .field("Enabled", &self.is_enabled());

        cfg_select! {
            target_arch = "x86_64" => {
                debug_struct.field("Huge", &self.is_huge());
            }

            target_arch = "riscv64" => {
                debug_struct.field("Global", &self.is_global());
            }
        }

        debug_struct
            .field("Permissions", &self.get_permissions())
            .field("User Accessible", &self.is_user_accessible())
            .finish()
    }
}
