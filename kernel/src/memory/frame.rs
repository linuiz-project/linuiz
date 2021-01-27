use core::ops::Range;
use x86_64::PhysAddr;

#[repr(transparent)]
#[derive(Clone)]
pub struct Frame(usize);

impl Frame {
    pub const fn null() -> Self {
        Self { 0: 0 }
    }

    pub const fn from_index(index: usize) -> Self {
        Self { 0: index }
    }

    pub fn from_addr(phys_addr: PhysAddr) -> Self {
        let addr_usize = phys_addr.as_u64() as usize;

        if (addr_usize & !0x000FFFFF_FFFFF000) > 0 {
            panic!("frame address format is invalid: {:?}", phys_addr)
        }

        Self {
            0: addr_usize / 0x1000,
        }
    }

    pub const fn index(&self) -> usize {
        self.0
    }

    pub const fn addr(&self) -> PhysAddr {
        PhysAddr::new_truncate((self.0 as u64) * 0x1000)
    }

    pub fn range_inclusive(range: Range<usize>) -> FrameIterator {
        FrameIterator::new(
            Frame::from_addr(PhysAddr::new(range.start as u64)),
            Frame::from_addr(PhysAddr::new(range.end as u64)),
        )
    }

    pub fn range_count(start_addr: PhysAddr, count: usize) -> FrameIterator {
        FrameIterator::new(
            Frame::from_addr(start_addr),
            Frame::from_addr(start_addr + (((count - 1) * 0x1000) as u64)),
        )
    }
}

pub struct FrameIterator {
    current: Frame,
    end: Frame,
}

impl FrameIterator {
    pub fn new(start: Frame, end: Frame) -> Self {
        if start.addr() >= end.addr() {
            panic!("start address must be less than end address");
        }

        Self {
            current: start,
            end,
        }
    }
}

impl Iterator for FrameIterator {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.0 <= self.end.0 {
            let frame = self.current.clone();
            self.current.0 += 1;
            Some(frame)
        } else {
            None
        }
    }
}

impl core::fmt::Debug for Frame {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Frame").field(&self.addr()).finish()
    }
}
