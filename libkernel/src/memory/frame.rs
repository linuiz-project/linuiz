use crate::{addr_ty::Physical, Address};

pub trait FrameIndexIterator: core::ops::RangeBounds<usize> + Iterator<Item = usize> {}
impl<T> FrameIndexIterator for T where T: core::ops::RangeBounds<usize> + Iterator<Item = usize> {}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Frame {
    index: usize,
}

impl Frame {
    pub const fn null() -> Self {
        Self { index: 0 }
    }

    /// TODO stop using range operator syntax for frames, introduce frame iterator
    ///
    /// Frame iterator only creatable by FrameAllocator (?)

    /// Creates a frame representing the specified frame index
    ///
    /// Note: usually offsets in 0x1000 steps.
    ///
    /// Safety: Frame creation should be deterministic. This concept is explained
    ///     in the `FrameAllocator` documentation.
    pub const unsafe fn from_index(index: usize) -> Self {
        Self { index }
    }

    pub const unsafe fn from_addr(addr: Address<Physical>) -> Self {
        if addr.is_aligned(0x1000) {
            Self {
                index: addr.as_usize() / 0x1000,
            }
        } else {
            panic!("frame address format is invalid")
        }
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn addr(&self) -> Address<Physical> {
        unsafe { Address::<Physical>::new_unsafe(self.index * 0x1000) }
    }

    pub fn into_iter(self) -> FrameIterator {
        FrameIterator::new(
            self,
            Self {
                index: self.index() + 1,
            },
        )
    }
}

impl PartialEq for Frame {
    fn eq(&self, other: &Self) -> bool {
        self.index() == other.index()
    }

    fn ne(&self, other: &Self) -> bool {
        self.index() != other.index()
    }
}

impl PartialOrd for Frame {
    fn lt(&self, other: &Self) -> bool {
        self.index() < other.index()
    }

    fn le(&self, other: &Self) -> bool {
        self.index() <= other.index()
    }

    fn gt(&self, other: &Self) -> bool {
        self.index() > other.index()
    }

    fn ge(&self, other: &Self) -> bool {
        self.index() >= other.index()
    }

    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.index().partial_cmp(&other.index())
    }
}

impl core::fmt::Debug for Frame {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Frame").field(&self.index()).finish()
    }
}

pub struct FrameIterator {
    start: Frame,
    end: Frame,
    current: Frame,
}

impl FrameIterator {
    pub(in crate::memory) const fn new(start: Frame, end: Frame) -> Self {
        Self {
            start,
            current: start,
            end,
        }
    }

    pub fn start(&self) -> &Frame {
        &self.start
    }

    pub fn current(&self) -> &Frame {
        &self.current
    }

    pub fn end(&self) -> &Frame {
        &self.end
    }

    pub fn reset(&mut self) {
        self.current = self.start;
    }
}

impl Iterator for FrameIterator {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            let next = Frame {
                index: self.current.index(),
            };
            self.current.index += 1;

            Some(next)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.start().index(), Some(self.end().index()))
    }
}

impl ExactSizeIterator for FrameIterator {
    fn len(&self) -> usize {
        self.end().index() - self.start().index()
    }
}

impl core::fmt::Debug for FrameIterator {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FrameIterator")
            .field("Start", self.start())
            .field("Current", self.current())
            .field("End", self.end())
            .finish()
    }
}
