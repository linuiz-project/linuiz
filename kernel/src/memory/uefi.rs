use crate::memory::{Frame, FrameIterator, MemoryType};
use x86_64::{PhysAddr, VirtAddr};

bitflags::bitflags! {
    pub struct UEFIMemoryAttribute: u64 {
        const UNCACHEABLE = 0x1;
        const WRITE_COMBINE = 0x2;
        const WRITE_THROUGH = 0x4;
        const WRITE_BACK = 0x8;
        const UNCACHABLE_EXPORTED = 0x10;
        const WRITE_PROTECT = 0x1000;
        const READ_PROTECT = 0x2000;
        const EXECUTE_PROTECT = 0x4000;
        const NON_VOLATILE = 0x8000;
        const MORE_RELIABLE = 0x10000;
        const READ_ONLY = 0x20000;
        const RUNTIME = 0x8000_0000_0000_0000;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UEFIMemoryDescriptor {
    pub ty: MemoryType,
    padding: u32,
    pub phys_start: PhysAddr,
    pub virt_start: VirtAddr,
    pub page_count: u64,
    pub att: UEFIMemoryAttribute,
}

impl UEFIMemoryDescriptor {
    pub fn range(&self) -> core::ops::Range<u64> {
        let addr_u64 = self.phys_start.as_u64();
        addr_u64..(addr_u64 + (self.page_count * 0x1000))
    }

    pub fn frame_iter(&self) -> FrameIterator {
        FrameIterator::new(
            Frame::from_addr(self.phys_start),
            Frame::from_addr(self.phys_start + self.page_count * 0x1000),
        )
    }
}
