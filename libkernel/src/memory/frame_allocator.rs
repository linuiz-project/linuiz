use crate::{memory::Frame, BitValue, RwBitArray};
use num_enum::TryFromPrimitive;
use spin::RwLock;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum FrameType {
    Free = 0,
    Locked,
    Reserved,
    Stack,
    NonUsable,
}

impl BitValue for FrameType {
    const BIT_WIDTH: usize = 0x4;
    const MASK: usize = 0xF;

    fn from_usize(value: usize) -> Self {
        use core::convert::TryFrom;

        match FrameType::try_from(value) {
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
    ExpectedFrameType(usize, FrameType),
    FreeWithAcquire,
}

struct FrameAllocatorMemory {
    total_memory: usize,
    free_memory: usize,
    locked_memory: usize,
    reserved_memory: usize,
}

pub struct FrameAllocator<'arr> {
    memory_map: RwBitArray<'arr, FrameType>,
    memory: RwLock<[usize; FrameType::MASK + 1]>,
}

impl<'arr> FrameAllocator<'arr> {
    /// Provides a hint as to the total memory usage (in frames, i.e. 0x1000 aligned)
    ///  a frame allocator will use given a specified total amount of memory.
    pub fn frame_count_hint(total_memory: usize) -> usize {
        assert_eq!(
            total_memory % 0x1000,
            0,
            "system memory should be page-aligned"
        );

        let frame_count = total_memory / 0x1000;
        crate::align_up_div(
            // each index of a RwBitArray is a `usize`, so multiply the index count with the `size_of` a `usize`
            (frame_count * FrameType::BIT_WIDTH) / 8, /* 8 bits per byte */
            // divide total RwBitArray memory size by frame size to get total frame count
            0x1000,
        )
    }

    pub unsafe fn from_ptr(base_ptr: *mut usize, total_memory: usize) -> Self {
        assert_eq!(
            base_ptr.align_offset(0x1000),
            0,
            "frame allocator base pointer must be frame-aligned"
        );
        assert_eq!(
            total_memory % 0x1000,
            0,
            "system memory should be page-aligned"
        );

        let this = Self {
            memory_map: RwBitArray::from_slice(&mut *core::ptr::slice_from_raw_parts_mut(
                base_ptr,
                RwBitArray::<FrameType>::length_hint(total_memory / 0x1000),
            )),
            memory: RwLock::new([0; FrameType::MASK + 1]),
        };

        let base_frame = Frame::from_addr(x86_64::PhysAddr::new(base_ptr as u64));
        let frame_iterator = Frame::range_count(base_frame, Self::frame_count_hint(total_memory));
        debug!(
            "Frame allocator defined with iterator: {:?}",
            frame_iterator
        );
        this.reserve_frames(frame_iterator)
            .expect("unexpectedly failed to reserve frame allocator frames");

        this
    }

    /// Total memory of a given type represented by frame allocator. If `None` is
    ///  provided for type, the total of all memory types is returned instead.
    pub fn total_memory(&self, of_type: Option<FrameType>) -> usize {
        let mem_read = self.memory.read();
        match of_type {
            Some(frame_type) => mem_read[frame_type.as_usize()],
            None => mem_read.last().unwrap(),
        }
    }

    /* FREE / LOCK / RESERVE / STACK - SINGLE */

    /// Attempts to free a specific frame in the allocator.
    pub unsafe fn free_frame(&self, frame: Frame) -> Result<(), FrameAllocatorError> {
        if self
            .memory_map
            .set_eq(frame.index(), FrameType::Free, FrameType::Locked)
        {
            let mut mem_write = self.memory.write();
            mem_write[FrameType::Free.as_usize()] += 0x1000;
            mem_write[FrameType::Locked.as_usize()] -= 0x1000;

            trace!(
                "Freed frame {}: {:?} -> {:?}",
                frame.index(),
                FrameType::Locked,
                FrameType::Free
            );
            Ok(())
        } else {
            Err(FrameAllocatorError::ExpectedFrameType(
                frame.index(),
                FrameType::Locked,
            ))
        }
    }

    pub unsafe fn acquire_frame(
        &self,
        index: usize,
        acq_type: FrameType,
    ) -> Result<Frame, FrameAllocatorError> {
        if acq_type == FrameType::Free {
            Err(FrameAllocatorError::FreeWithAcquire)
        } else if self.memory_map.set_eq(index, acq_type, FrameType::Free) {
            let mut mem_write = self.memory.write();
            mem_write[FrameType::Free.as_usize()] -= 0x1000;
            mem_write[acq_type.as_usize()] += 0x1000;

            trace!(
                "Acquired frame {}: {:?} -> {:?}",
                index,
                FrameType::Free,
                acq_type,
            );

            Ok(Frame::from_index(index))
        } else {
            Err(FrameAllocatorError::ExpectedFrameType(
                index,
                FrameType::Free,
            ))
        }
    }

    /* FREE / LOCK / RESERVE / STACK - ITER */

    /// Attempts to free many frames from an iterator.
    pub unsafe fn free_frames(
        &self,
        frames: core::ops::Range<Frame>,
    ) -> Result<(), FrameAllocatorError> {
        for frame in frames {
            if let Err(error) = self.free_frame(frame) {
                return Err(error);
            }
        }

        Ok(())
    }

    pub unsafe fn acquire_frames(
        &self,
        frame_indexes: core::ops::Range<usize>,
        acq_type: FrameType,
    ) -> Result<core::ops::Range<Frame>, FrameAllocatorError> {
        for index in frame_indexes {
            if let Err(error) = self.acquire_frame(index, acq_type) {
                return Err(error);
            }
        }

        Ok(Frame::from_index(frame_indexes.start)..Frame::from_index(frame_indexes.end - 1))
    }

    /// Attempts to iterate the allocator's frames, and returns the first unallocated frame.
    pub fn lock_next(&self) -> Option<Frame> {
        self.memory_map
            .set_eq_next(FrameType::Locked, FrameType::Free)
            .map(|index| {
                debug_assert_eq!(
                    self.memory_map.get(index),
                    FrameType::Locked,
                    "failed to allocate next frame"
                );

                let mut mem_write = self.memory.write();
                mem_write[FrameType::Free.as_usize()] -= 0x1000;
                mem_write[FrameType::Locked.as_usize()] += 0x1000;

                let frame = Frame::from_index(index);
                trace!("Locked next free frame: {:?}", frame);
                frame
            })
    }

    /// Executes a given callback function, passing frame data from each frame the
    ///  allocator represents.
    pub fn iter_callback<F>(&self, mut callback: F)
    where
        F: FnMut(usize, FrameType),
    {
        for index in 0..self.memory_map.len() {
            callback(index, self.memory_map.get(index));
        }
    }

    #[cfg(debug_assertions)]
    pub fn debug_log_elements(&self) {
        self.memory_map.debug_log_elements();
    }
}
