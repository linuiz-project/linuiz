pub mod paging;

use x86_64::PhysAddr;

pub const PAGE_SIZE: u64 = 0x1000; // 4096
pub const KIBIBYTE: u64 = 0x400; // 1024
pub const MIBIBYTE: u64 = KIBIBYTE * KIBIBYTE;

pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame>;
    fn deallocate_frame(&mut self, frame: Frame);
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    number: u64,
}

impl Frame {
    pub const fn new(address: u64) -> Self {
        Self {
            number: address / PAGE_SIZE,
        }
    }

    pub fn address(&self) -> PhysAddr {
        PhysAddr::new(self.number * PAGE_SIZE)
    }
}

pub fn to_kibibytes(value: u64) -> u64 {
    value / KIBIBYTE
}

pub fn to_mibibytes(value: u64) -> u64 {
    value / MIBIBYTE
}
