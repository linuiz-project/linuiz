use x86_64::{PhysAddr, VirtAddr};

use crate::structures::memory::{
    paging::{PageAttributes, PageDescriptor},
    Frame,
};

use super::{Page, PageFrameAllocator, ADDRESS_SHIFT};

#[repr(C, align(0x1000))]
pub struct PageTable {
    descriptors: [PageDescriptor; 512],
}

impl PageTable {
    const fn get_index_from_addr(addr: u64) -> u64 {
        addr & 0x1FF
    }

    pub const fn new() -> Self {
        Self {
            descriptors: [PageDescriptor::unused(); 512],
        }
    }

    pub fn clear(&mut self) {
        for descriptor in self.descriptors.iter_mut() {
            descriptor.set_unused();
        }
    }

    pub fn get_page(
        &mut self,
        addr: VirtAddr,
        allocator: &mut PageFrameAllocator,
    ) -> Result<(), ()> {
        self.get_page_internal(addr.as_u64(), allocator, 4)
    }

    fn get_page_internal(
        &mut self,
        addr: u64,
        allocator: &mut PageFrameAllocator,
        depth: u8,
    ) -> Result<(), ()> {
        let descriptor = &mut self.descriptors[addr as usize];
        if !descriptor.attribs().contains(PageAttributes::PRESENT) {
            trace!("Table descriptor doesn't point to an allocated frame.");
            trace!("Attempting to allocate frame for descriptor.");

            match allocator.next_free() {
                Some(mut frame) => {
                    descriptor.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
                    unsafe { frame.clear() };
                }
                None => return Err(()), // todo error
            }
        }

        let frame = descriptor.frame().expect("failed to get frame");

        if depth > 0 {
            let sub_table = unsafe { &mut *(frame.addr().as_u64() as *mut PageTable) };
            return sub_table.get_page_internal(addr >> 9, allocator, depth - 1);
        } else {
            info!("{:?}", frame.addr());
            return Ok(());
        }
    }
}
