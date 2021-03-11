use crate::{addr_ty::Virtual, cell::SyncRefCell, memory::Frame, Address};
use num_enum::TryFromPrimitive;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum FrameState {
    Free = 0,
    Locked,
    Reserved,
    NonUsable,
    MMIO,
}

impl crate::BitValue for FrameState {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAllocatorError {
    ExpectedFrameState(usize, FrameState),
    NonMMIOFrameState(usize, FrameState),
    FreeWithAcquire,
}

pub struct FrameStateIterator<'iter> {
    iterator: &'iter mut dyn Iterator<Item = FrameState>,
}

impl<'iter> FrameStateIterator<'iter> {
    pub fn new(iterator: &'iter mut impl Iterator<Item = FrameState>) -> Self {
        Self { iterator }
    }
}

impl Iterator for FrameStateIterator<'_> {
    type Item = FrameState;

    fn next(&mut self) -> Option<Self::Item> {
        self.iterator.next()
    }
}

pub trait FrameAllocator {
    unsafe fn acquire_frame(
        &self,
        index: usize,
        acq_state: FrameState,
    ) -> Result<Frame, FrameAllocatorError>;
    unsafe fn free_frame(&self, frame: Frame) -> Result<(), FrameAllocatorError>;

    unsafe fn acquire_frames(
        &self,
        index: usize,
        count: usize,
        acq_state: FrameState,
    ) -> Result<crate::memory::FrameIterator, FrameAllocatorError>;
    unsafe fn free_frames(
        &self,
        frames: crate::memory::FrameIterator,
    ) -> Result<(), FrameAllocatorError>;
    fn lock_next(&self) -> Option<Frame>;

    fn total_memory(&self, mem_state: Option<FrameState>) -> usize;

    fn iter(&self) -> FrameStateIterator;
}

static DEFAULT_FALLOCATOR: SyncRefCell<&'static dyn FrameAllocator> = SyncRefCell::new();

pub fn set(allocator: &'static dyn FrameAllocator) {
    DEFAULT_FALLOCATOR.set(allocator)
}

pub fn get() -> &'static dyn FrameAllocator {
    DEFAULT_FALLOCATOR
        .get()
        .expect("frame allocator has not been set")
}

pub fn virtual_map_offset() -> Address<Virtual> {
    Address::<Virtual>::new(crate::VADDR_HW_MAX - get().total_memory(None))
}
