use x86_64::{PhysAddr, VirtAddr};

use crate::structures::memory::paging::{PageFrameAllocator, PageTable};

pub struct PageTableManager<'palloc> {
    frame_allocator: PageFrameAllocator<'palloc>,
    page_table: PageTable,
}

impl<'palloc> PageTableManager<'palloc> {
    pub fn new(frame_allocator: PageFrameAllocator<'palloc>) -> Self {
        Self {
            frame_allocator,
            page_table: PageTable::new(),
        }
    }

    pub fn map_memory(&mut self, virt_addr: VirtAddr, phys_addr: PhysAddr) {
        let aaa = self
            .page_table
            .get_page(virt_addr, &mut self.frame_allocator);
    }
}
