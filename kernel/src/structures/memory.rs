use x86_64::PhysAddr;

pub const PAGE_SIZE: u64 = 0x1000;

pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame>;
    fn deallocate_frame(&mut self, frame: Frame);
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    number: u64,
}

impl Frame {
    const fn new(address: u64) -> Self {
        Self {
            number: address / PAGE_SIZE,
        }
    }

    const fn address(&self) -> PhysAddr {
        PhysAddr::new(self.number * PAGE_SIZE)
    }
}
