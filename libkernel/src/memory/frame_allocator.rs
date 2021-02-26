use crate::{
    memory::{Frame, FrameIndexIterator},
    BitValue, RwBitArray,
};
use num_enum::TryFromPrimitive;
use spin::RwLock;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum FrameState {
    Free = 0,
    Locked,
    Reserved,
    Stack,
    NonUsable,
    MMIO,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAllocatorError {
    ExpectedFrameType(usize, FrameState),
    FreeWithAcquire,
    MMIOWithinRAM,
}

pub struct FrameAllocator<'arr> {
    memory_map: RwBitArray<'arr, FrameState>,
    memory: RwLock<[usize; FrameState::MASK + 1]>,
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
            (frame_count * FrameState::BIT_WIDTH) / 8, /* 8 bits per byte */
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

        let mut memory_counters = [0; FrameState::MASK + 1];
        memory_counters[FrameState::Free.as_usize()] = total_memory;
        memory_counters[FrameState::MASK] = total_memory;

        let this = Self {
            memory_map: RwBitArray::from_slice(&mut *core::ptr::slice_from_raw_parts_mut(
                base_ptr,
                RwBitArray::<FrameState>::length_hint(total_memory / 0x1000),
            )),
            memory: RwLock::new(memory_counters),
        };

        let base_frame_index = (base_ptr as usize) / 0x1000;
        let end_frame_index = base_frame_index + Self::frame_count_hint(total_memory);
        let frame_range = base_frame_index..=end_frame_index;
        debug!("Frame allocator defined with iterator: {:?}", frame_range);
        this.acquire_frames(frame_range, FrameState::Reserved)
            .expect("unexpectedly failed to reserve frame allocator frames");

        this
    }

    /// Total memory of a given type represented by frame allocator. If `None` is
    ///  provided for type, the total of all memory types is returned instead.
    pub fn total_memory(&self, of_type: Option<FrameState>) -> usize {
        let mem_read = self.memory.read();
        match of_type {
            Some(frame_type) => mem_read[frame_type.as_usize()],
            None => *mem_read.last().unwrap(),
        }
    }

    /* FREE / LOCK / RESERVE / STACK - SINGLE */

    /// Attempts to free a specific frame in the allocator.
    pub unsafe fn free_frame(&self, frame: Frame) -> Result<(), FrameAllocatorError> {
        if self
            .memory_map
            .set_eq(frame.index(), FrameState::Free, FrameState::Locked)
        {
            let mut mem_write = self.memory.write();
            mem_write[FrameState::Free.as_usize()] += 0x1000;
            mem_write[FrameState::Locked.as_usize()] -= 0x1000;

            trace!(
                "Freed frame {}: {:?} -> {:?}",
                frame.index(),
                FrameState::Locked,
                FrameState::Free
            );
            Ok(())
        } else {
            Err(FrameAllocatorError::ExpectedFrameType(
                frame.index(),
                FrameState::Locked,
            ))
        }
    }

    pub unsafe fn coerce_frame(&self, frame: Frame, new_state: FrameState) {
        let old_state = self.memory_map.get(frame.index());
        self.memory_map.set(frame.index(), new_state);

        let mut mem_write = self.memory.write();
        mem_write[old_state.as_usize()] -= 0x1000;
        mem_write[new_state.as_usize()] += 0x1000;

        debug!(
            "Forcibly freed frame {}: {:?} -> {:?}",
            frame.index(),
            old_state,
            new_state
        );
    }

    pub unsafe fn acquire_frame(
        &self,
        index: usize,
        acq_type: FrameState,
    ) -> Result<Frame, FrameAllocatorError> {
        match acq_type {
            FrameState::Free => Err(FrameAllocatorError::FreeWithAcquire),
            FrameState::MMIO => {
                if (index * 0x1000) > self.total_memory(None) {
                    Ok(Frame::from_index(index))
                } else {
                    Err(FrameAllocatorError::MMIOWithinRAM)
                }
            }
            _ if self.memory_map.set_eq(index, acq_type, FrameState::Free) => {
                let mut mem_write = self.memory.write();
                mem_write[FrameState::Free.as_usize()] -= 0x1000;
                mem_write[acq_type.as_usize()] += 0x1000;

                trace!(
                    "Acquired frame {}: {:?} -> {:?}",
                    index,
                    FrameState::Free,
                    acq_type,
                );

                Ok(Frame::from_index(index))
            }
            _ => Err(FrameAllocatorError::ExpectedFrameType(
                index,
                FrameState::Free,
            )),
        }
    }

    /* FREE / LOCK / RESERVE / STACK - ITER */

    /// Attempts to free many frames from an iterator.
    pub unsafe fn free_frames(
        &self,
        frames: impl Iterator<Item = Frame>,
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
        frame_indexes: impl FrameIndexIterator,
        acq_type: FrameState,
    ) -> Result<core::ops::Range<Frame>, FrameAllocatorError> {
        let start_frame = Frame::from_index(match frame_indexes.start_bound() {
            core::ops::Bound::Included(start) => *start,
            core::ops::Bound::Excluded(start) => *start - 1,
            _ => panic!("failed to calculate start bound for frame range"),
        });

        let end_frame = Frame::from_index(match frame_indexes.end_bound() {
            core::ops::Bound::Included(end) => *end,
            core::ops::Bound::Excluded(end) => *end - 1,
            _ => panic!("failed to calculate start bound for frame range"),
        });

        for index in frame_indexes {
            if let Err(error) = self.acquire_frame(index, acq_type) {
                return Err(error);
            }
        }

        Ok(start_frame..end_frame)
    }

    /// Attempts to iterate the allocator's frames, and returns the first unallocated frame.
    pub fn lock_next(&self) -> Option<Frame> {
        self.memory_map
            .set_eq_next(FrameState::Locked, FrameState::Free)
            .map(|index| {
                debug_assert_eq!(
                    self.memory_map.get(index),
                    FrameState::Locked,
                    "failed to allocate next frame"
                );

                let mut mem_write = self.memory.write();
                mem_write[FrameState::Free.as_usize()] -= 0x1000;
                mem_write[FrameState::Locked.as_usize()] += 0x1000;

                let frame = Frame::from_index(index);
                trace!("Locked next free frame: {:?}", frame);
                frame
            })
    }

    /// Executes a given callback function, passing frame data from each frame the
    ///  allocator represents.
    pub fn iter_callback<F>(&self, mut callback: F)
    where
        F: FnMut(usize, FrameState),
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
