use x86_64::PhysAddr;

#[repr(transparent)]
#[derive(Clone, Copy)]
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

        if (addr_usize & !0x000FFFFF_FFFFF000) > 0 {
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

impl core::fmt::Debug for Frame {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Frame").field(&self.index()).finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FrameIterator {
    current: Frame,
    end: Frame,
}

impl FrameIterator {
    pub const fn remaining(&self) -> usize {
        self.end.index() - self.current.index()
    }
}

impl Iterator for FrameIterator {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.0 < self.end.0 {
            let frame = self.current.clone();
            self.current.0 += 1;
            Some(frame)
        } else {
            None
        }
    }
}
