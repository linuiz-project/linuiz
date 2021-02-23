use crate::BitValue;
use num_enum::TryFromPrimitive;
use x86_64::PhysAddr;

pub trait FrameIterator: core::ops::RangeBounds<Frame> + Iterator<Item = Frame> {}
impl<T> FrameIterator for T where T: core::ops::RangeBounds<Frame> + Iterator<Item = Frame> {}

pub trait FrameIndexIterator: core::ops::RangeBounds<usize> + Iterator<Item = usize> {}
impl<T> FrameIndexIterator for T where T: core::ops::RangeBounds<usize> + Iterator<Item = usize> {}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum FrameState {
    Free = 0,
    Locked,
    Reserved,
    Stack,
    NonUsable,
}

impl BitValue for FrameState {
    const BIT_WIDTH: usize = 0x4;
    const MASK: usize = 0xF;

    fn from_usize(value: usize) -> Self {
        use core::convert::TryFrom;

        match FrameState::try_from(value) {
            Ok(frame_type) => frame_type,
            Err(err) => panic!("invalid value for frame type: {:?}", err),
        }
    }

    fn as_usize(&self) -> usize {
        *self as usize
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    index: usize,
}

impl Frame {
    #[inline]
    pub const fn null() -> Self {
        Self { index: 0 }
    }

    #[inline]
    pub const fn from_index(index: usize) -> Self {
        Self { index }
    }

    #[inline]
    pub fn from_addr(phys_addr: PhysAddr) -> Self {
        let addr_usize = phys_addr.as_u64() as usize;

        if (addr_usize & 0xFFF) > 0 {
            panic!("frame address format is invalid: {:?}", phys_addr)
        }

        Self {
            index: addr_usize / 0x1000,
        }
    }

    #[inline]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub const fn addr(&self) -> PhysAddr {
        PhysAddr::new_truncate(self.addr_u64())
    }

    #[inline]
    pub const fn addr_u64(&self) -> u64 {
        (self.index as u64) * 0x1000
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
