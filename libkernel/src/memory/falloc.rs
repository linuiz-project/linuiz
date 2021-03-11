use crate::cell::SyncRefCell;
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

pub trait FrameAllocator {
    fn acquire_frame(&self, index: usize, acq_state: FrameState);
    fn free_frame(&self, index: usize);

    fn acquire_frames(&self, index: usize, count: usize);
    fn free_frames(&self, frames: crate::memory::FrameIterator);

    fn total_memory(&self, mem_state: FrameState) -> usize;
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
