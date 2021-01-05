pub mod paging;

use x86_64::PhysAddr;

pub const PAGE_SIZE: usize = 0x1000; // 4096
pub const KIBIBYTE: usize = 0x400; // 1024
pub const MIBIBYTE: usize = KIBIBYTE * KIBIBYTE;

pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame>;
    fn deallocate_frame(&mut self, frame: Frame);
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    number: usize,
}

impl Frame {
    pub const fn new(address: usize) -> Self {
        Self {
            number: address / PAGE_SIZE,
        }
    }

    pub fn address(&self) -> PhysAddr {
        PhysAddr::new((self.number * PAGE_SIZE) as u64)
    }
}

pub fn to_kibibytes(value: usize) -> usize {
    value / KIBIBYTE
}

pub fn to_mibibytes(value: usize) -> usize {
    value / MIBIBYTE
}
