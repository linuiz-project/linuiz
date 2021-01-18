use crate::structures::memory::{
    paging::{Level4, PageAttributes, PageTable, TableLevel},
    Frame,
};
use x86_64::VirtAddr;

pub struct PageTableManager {
    page_table: PageTable<Level4>,
}

impl PageTableManager {
    pub fn new(page_table_frame: Frame) -> Self {
        let mut this = Self {
            page_table: PageTable::new(),
        };

        let mut entry = this.page_table[511];
        entry.set(
            &page_table_frame,
            PageAttributes::PRESENT | PageAttributes::WRITABLE,
        );

        this
    }

    pub fn map(&mut self, virt_addr: VirtAddr, frame: &Frame) {
        assert_eq!(
            virt_addr.as_u64() % 0x1000,
            0,
            "address must be page-aligned"
        );

        let addr_u64 = (virt_addr.as_u64() >> 12) as usize;
        let entry = &mut self
            .page_table
            .sub_table_mut((addr_u64 >> 27) & 0x1FF)
            .sub_table_mut((addr_u64 >> 18) & 0x1FF)
            .sub_table_mut((addr_u64 >> 9) & 0x1FF)[(addr_u64 >> 0) & 0x1FF];
        if entry.is_present() {
            crate::instructions::tlb::invalidate(virt_addr);
        }

        entry.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
        trace!("Mapped {:?} to {:?}: {:?}", virt_addr, entry.frame(), entry);
    }

    pub fn identity_map(&mut self, frame: &Frame) {
        self.map(VirtAddr::new(frame.addr().as_u64()), frame);
    }

    pub fn unmap(&mut self, virt_addr: VirtAddr) {
        assert_eq!(
            virt_addr.as_u64() % 0x1000,
            0,
            "address must be page-aligned"
        );

        let addr_u64 = (virt_addr.as_u64() >> 12) as usize;
        let entry = &mut self
            .page_table
            .sub_table_mut((addr_u64 >> 27) & 0x1FF)
            .sub_table_mut((addr_u64 >> 18) & 0x1FF)
            .sub_table_mut((addr_u64 >> 9) & 0x1FF)[(addr_u64 >> 0) & 0x1FF];
        entry.set_nonpresent();
        trace!(
            "Unmapped {:?} from {:?}: {:?}",
            virt_addr,
            entry.frame(),
            entry
        );
    }

    pub fn write_pml4(&self) {
        let frame = &self.page_table.frame();
        info!(
            "Writing page table manager's PML4 to CR3 register: {:?}.",
            frame
        );

        unsafe { crate::registers::CR3::write(frame, None) };
    }
}
