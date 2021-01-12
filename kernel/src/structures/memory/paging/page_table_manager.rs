use crate::structures::memory::{
    paging::{PageAttributes, PageTable},
    Frame,
};
use x86_64::VirtAddr;

use super::frame_allocator::global_allocator;

pub struct PageTableManager {
    page_table: PageTable,
}

impl PageTableManager {
    pub fn new() -> Self {
        Self {
            page_table: PageTable::new(),
        }
    }

    pub fn map_memory(&mut self, virt_addr: VirtAddr, frame: &Frame) {
        assert_eq!(virt_addr.as_u64() % 0x1000, 0);

        let addr = (virt_addr.as_u64() >> 12) as usize;
        let mut page_table = &mut self.page_table;

        for iter_index in (0..4).rev() {
            let descriptor = &mut self.page_table[(addr >> (iter_index * 9)) & 0x1FF];

            let mut clear_page_table = false;
            if !descriptor.attribs().contains(PageAttributes::PRESENT) {
                trace!("Descriptor doesn't point to an allocated frame, so one will be allocated.");

                match global_allocator(|allocator| allocator.allocate_next()) {
                    Some(mut frame) => {
                        descriptor.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
                        unsafe {
                            frame.clear();
                        }
                    }
                    None => panic!("failed to lock a frame for new page table"),
                }

                trace!("Allocated descriptor: {:?}", descriptor);
                clear_page_table = true;
            }

            page_table = unsafe { descriptor.as_page_table_mut() };
            if clear_page_table {
                page_table.clear();
            }
        }

        // At this point, `page_table` should be the L1 page table.
        let descriptor = &mut page_table[addr & 0x1FF];
        descriptor.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
        trace!("Mapped {:?} to {:?}: {:?}", virt_addr, frame, descriptor);
    }
}
