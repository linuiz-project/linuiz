use crate::structures::memory::paging::{FrameAllocator, PageAttributes, PageDescriptor};
use core::ops::{Index, IndexMut};

#[repr(C, align(0x1000))]
pub struct PageTable {
    descriptors: [PageDescriptor; 512],
}

impl PageTable {
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

    pub fn allocated_descriptor(
        &mut self,
        index: usize,
        frame_allocator: &mut FrameAllocator,
    ) -> &mut PageDescriptor {
        let descriptor = &mut self[index];
        trace!("Found descriptor for index {}: {:?}", index, descriptor);

        if !descriptor.attribs().contains(PageAttributes::PRESENT) {
            trace!("Descriptor doesn't point to an allocated frame, so one will be allocated.");

            match frame_allocator.allocate_next() {
                Some(mut frame) => {
                    descriptor.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
                    unsafe {
                        frame.clear();
                    }
                }
                None => panic!("failed to lock a frame for new page table"),
            }

            trace!("Allocated descriptor: {:?}", descriptor);
        }

        descriptor
    }
}

impl Index<usize> for PageTable {
    type Output = PageDescriptor;

    fn index(&self, index: usize) -> &Self::Output {
        &self.descriptors[index]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.descriptors[index]
    }
}
