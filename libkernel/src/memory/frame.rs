use x86_64::PhysAddr;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame(usize);

impl Frame {
    #[inline]
    pub const fn null() -> Self {
        Self { 0: 0 }
    }

    #[inline]
    pub const fn from_index(index: usize) -> Self {
        Self { 0: index }
    }

    #[inline]
    pub fn from_addr(phys_addr: PhysAddr) -> Self {
        let addr_usize = phys_addr.as_u64() as usize;

        if (addr_usize & 0xFFF) > 0 {
            panic!("frame address format is invalid: {:?}", phys_addr)
        }

        Self {
            0: addr_usize / 0x1000,
        }
    }

    #[inline]
    pub const fn index(&self) -> usize {
        self.0
    }

    #[inline]
    pub const fn addr(&self) -> PhysAddr {
        PhysAddr::new_truncate(self.addr_u64())
    }

    #[inline]
    pub const fn addr_u64(&self) -> u64 {
        (self.0 as u64) * 0x1000
    }

    pub fn range_count(start: Frame, count: usize) -> FrameIterator {
        FrameIterator {
            current: start,
            end: Frame::from_index(start.index() + count),
        }
    }
}

unsafe impl core::iter::Step for Frame {
    fn forward(start: Self, count: usize) -> Self {
        Self::from_index(start.index() + count)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let new_index = start.index() + count;
        if new_index >= start.index() {
            Some(Frame::from_index(new_index))
        } else {
            None
        }
    }

    fn backward(start: Self, count: usize) -> Self {
        Self::from_index(start.index() - count)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let new_index = start.index() - count;
        if new_index <= start.index() {
            Some(Frame::from_index(new_index))
        } else {
            None
        }
    }

    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if start.index() <= end.index() {
            Some(end.index() - start.index())
        } else {
            None
        }
    }
}

impl core::fmt::Debug for Frame {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Frame").field(&self.index()).finish()
    }
}
