use crate::structures::memory::{
    paging::{FrameAllocator, PageAttributes, PageTable},
    Frame,
};
use x86_64::VirtAddr;

pub struct PageTableManager<'palloc> {
    frame_allocator: FrameAllocator<'palloc>,
    page_table: PageTable,
}

impl<'palloc> PageTableManager<'palloc> {
    pub fn new(frame_allocator: FrameAllocator<'palloc>) -> Self {
        Self {
            frame_allocator,
            page_table: PageTable::new(),
        }
    }

    pub fn map_memory(&mut self, virt_addr: VirtAddr, frame: &Frame) -> Result<(), ()> {
        assert_eq!(virt_addr.as_u64() % 0x1000, 0);

        let addr = (virt_addr.as_u64() >> 12) as usize;
        let mut page_table = &mut self.page_table;

        for iter_index in (0..4).rev() {
            let index = (addr >> (iter_index * 9)) & 0x1FF;
            let descriptor = page_table.allocated_descriptor(index, &mut self.frame_allocator);
            let frame = descriptor.frame().expect("failed to get frame");
            page_table = unsafe { &mut *(frame.addr().as_u64() as *mut PageTable) };
        }

        // At this point, `page_table` should be the L1 page table.
        let descriptor = &mut page_table[addr & 0x1FF];
        descriptor.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
        trace!("Mapped {:?} to {:?}: {:?}", virt_addr, frame, descriptor);

        Ok(())
    }
}
