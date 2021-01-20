use crate::structures::memory::{
    global_allocator, global_allocator_mut,
    paging::{Level4, PageAttributes, PageTable},
    Frame, Page,
};
use x86_64::{PhysAddr, VirtAddr};

pub struct PageTableManager {
    pml4_ptr: *mut PageTable<Level4>,
}

impl PageTableManager {
    pub fn new() -> Self {
        let pml4_frame = global_allocator_mut(|allocator| allocator.lock_next())
            .expect("failed to lock frame for kernel's PML4");
        let pml4_addr = VirtAddr::new(/* phys_map_offset + */ pml4_frame.addr().as_u64());

        Self {
            pml4_ptr: pml4_addr.as_mut_ptr(),
        }
    }

    fn pml4(&self) -> &PageTable<Level4> {
        unsafe { &*self.pml4_ptr }
    }

    fn pml4_mut(&mut self) -> &mut PageTable<Level4> {
        unsafe { &mut *self.pml4_ptr }
    }

    pub fn map(&mut self, page: &Page, frame: &Frame) {
        let addr_usize = (page.addr().as_u64() >> 12) as usize;
        let entry = &mut self
            .pml4_mut()
            .sub_table_mut((addr_usize >> 27) & 0x1FF)
            .sub_table_mut((addr_usize >> 18) & 0x1FF)
            .sub_table_mut((addr_usize >> 9) & 0x1FF)[(addr_usize >> 0) & 0x1FF];
        if entry.is_present() {
            crate::instructions::tlb::invalidate(page);
        }

        entry.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
        trace!("Mapped {:?} to {:?}: {:?}", page, entry.frame(), entry);
    }

    pub fn unmap(&mut self, page: &Page) {
        let addr_usize = (page.addr().as_u64() >> 12) as usize;
        let entry = &mut self
            .pml4_mut()
            .sub_table_mut((addr_usize >> 27) & 0x1FF)
            .sub_table_mut((addr_usize >> 18) & 0x1FF)
            .sub_table_mut((addr_usize >> 9) & 0x1FF)[(addr_usize >> 0) & 0x1FF];
        entry.set_nonpresent();
        trace!("Unmapped {:?} from {:?}: {:?}", page, entry.frame(), entry);
    }

    pub fn identity_map(&mut self, frame: &Frame) {
        self.map(
            &Page::from_addr(VirtAddr::new(frame.addr().as_u64())),
            frame,
        );
    }

    pub fn identity_map_full(&mut self) {
        debug!("Identity mapping all available memory.");
        let total_memory = global_allocator(|allocator| allocator.total_memory());
        for addr in (0..total_memory).step_by(0x1000) {
            self.identity_map(&Frame::from_addr(PhysAddr::new(addr as u64)));
        }
    }

    pub fn write_pml4(&self) {
        let frame = &self.pml4().frame();
        info!(
            "Writing page table manager's PML4 to CR3 register: {:?}.",
            frame
        );

        unsafe { crate::registers::CR3::write(frame, None) };
    }
}
