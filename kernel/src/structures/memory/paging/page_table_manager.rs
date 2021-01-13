use crate::structures::memory::{
    paging::{
        page_table::{Level4, PageTable},
        PageAttributes,
    },
    Frame,
};
use x86_64::{
    structures::paging::{PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

pub struct PageTableManager {
    page_table: PageTable<Level4>,
}

impl PageTableManager {
    pub fn new() -> Self {
        Self {
            page_table: PageTable::new(),
        }
    }

    pub fn map_memory(&mut self, virt_addr: VirtAddr, frame: &Frame) {
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
        entry.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
        trace!("Mapped {:?} to {:?}: {:?}", virt_addr, frame, entry);
    }

    pub fn phys_frame(&mut self) -> PhysFrame<Size4KiB> {
        PhysFrame::from_start_address(PhysAddr::new(self.page_table.frame().addr().as_u64()))
            .expect("frame address is not properly aligned")
    }
}
