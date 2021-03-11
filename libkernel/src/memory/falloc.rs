use crate::{
    addr_ty::Virtual, cell::SyncOnceCell, memory::Frame, Address, BitValue, RwBitArray,
    RwBitArrayIterator,
};
use num_enum::TryFromPrimitive;
use spin::RwLock;

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

/// Structure for deterministically reserving, locking, and freeing RAM frames.
///
/// Deterministic Frame Lifetimes
/// -----------------------------
/// Deterministic frame lifetimes is the concept that a frame's lifetime should be
/// carefully controlled to ensure its behaviour matches how its used in hardware.
///
/// As an example, a page table entry locks a frame to create a sub-table. Conceptually,
/// the sub-table's entry in the page table owns the frame that sub-table exists on. It
/// follows that that entry should control the lifetime of that frame.
///
/// This example encapsulates the core idea, that the `Frame` struct shouldn't be instantiated
/// out of thin air. Its creation should be carefully controlled, to ensure each individual frame's
/// lifetime matches up with how it is used or consumed in hardware and software.
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

    unsafe fn from_ptr(base_ptr: *mut usize, total_memory: usize) -> Self {
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

        let total_frames = total_memory / 0x1000;
        let this = Self {
            memory_map: RwBitArray::from_slice(
                &mut *core::ptr::slice_from_raw_parts_mut(
                    base_ptr,
                    RwBitArray::<FrameState>::section_length_hint(total_frames),
                ),
                total_frames,
            ),
            memory: RwLock::new(memory_counters),
        };

        this.acquire_frames(
            (base_ptr as usize) / 0x1000,
            Self::frame_count_hint(total_memory),
            FrameState::Reserved,
        )
        .expect("unexpectedly failed to reserve frame allocator frames");

        this
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
            Err(FrameAllocatorError::ExpectedFrameState(
                frame.index(),
                FrameState::Locked,
            ))
        }
    }

    pub unsafe fn acquire_frame(
        &self,
        index: usize,
        acq_state: FrameState,
    ) -> Result<Frame, FrameAllocatorError> {
        match acq_state {
            FrameState::Free => Err(FrameAllocatorError::FreeWithAcquire),
            FrameState::MMIO => match self.memory_map.get(index) {
                cur_state if matches!(cur_state, FrameState::Reserved | FrameState::NonUsable) => {
                    self.memory_map.set(index, acq_state);

                    let mut mem_write = self.memory.write();
                    mem_write[cur_state.as_usize()] -= 0x1000;
                    mem_write[acq_state.as_usize()] += 0x1000;

                    Ok(Frame::from_index(index))
                }
                cur_state => Err(FrameAllocatorError::NonMMIOFrameState(index, cur_state)),
            },
            _ if self.memory_map.set_eq(index, acq_state, FrameState::Free) => {
                let mut mem_write = self.memory.write();
                mem_write[FrameState::Free.as_usize()] -= 0x1000;
                mem_write[acq_state.as_usize()] += 0x1000;

                Ok(Frame::from_index(index))
            }
            _ => Err(FrameAllocatorError::ExpectedFrameState(
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
        index: usize,
        count: usize,
        acq_state: FrameState,
    ) -> Result<crate::memory::FrameIterator, FrameAllocatorError> {
        let start_index = index;
        let end_index = index + count;
        for frame_index in start_index..end_index {
            if let Err(error) = self.acquire_frame(frame_index, acq_state) {
                return Err(error);
            }
        }

        Ok(crate::memory::FrameIterator::new(
            Frame::from_index(start_index),
            Frame::from_index(end_index),
        ))
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

                let frame = unsafe { Frame::from_index(index) };
                trace!("Locked next free frame: {:?}", frame);
                frame
            })
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

    pub fn iter<'outer>(&'arr self) -> RwBitArrayIterator<'outer, 'arr, FrameState> {
        self.memory_map.iter()
    }

    #[cfg(debug_assertions)]
    pub fn debug_log_elements(&self) {
        self.memory_map.debug_log_elements();
    }
}

static DEFAULT_FALLOCATOR: SyncOnceCell<FrameAllocator> = SyncOnceCell::new();

pub unsafe fn load(ptr: *mut usize, total_memory: usize) {
    if !DEFAULT_FALLOCATOR.get().is_some() {
        DEFAULT_FALLOCATOR
            .set(FrameAllocator::from_ptr(ptr, total_memory))
            .ok();
    } else {
        panic!("frame allocator has already been configured")
    }
}

pub fn get() -> &'static FrameAllocator<'static> {
    DEFAULT_FALLOCATOR
        .get()
        .expect("frame allocator has not been configured")
}

pub fn virtual_map_offset() -> Address<Virtual> {
    Address::<Virtual>::new(crate::VADDR_HW_MAX - get().total_memory(None))
}
