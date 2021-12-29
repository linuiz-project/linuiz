use crate::{
    addr_ty::Virtual, cell::SyncOnceCell, memory::Frame, Address, BitValue, BitValueArray,
    BitValueArrayIterator,
};
use spin::RwLock;

use super::FrameIterator;

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

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameState {
    Free = 0,
    Locked,
    /// Indicates a frame should never be used by software (except for MMIO).
    /// REMARK: Acquiring this will _NEVER_ fail, and so is extremely unsafe to do.
    ///     Use with caution.
    Reserved,
}

impl crate::BitValue for FrameState {
    const BIT_WIDTH: usize = 0x2;
    const MASK: usize = 0b11;

    fn from_usize(value: usize) -> Self {
        match value {
            0 => FrameState::Free,
            1 => FrameState::Locked,
            2 => FrameState::Reserved,
            _ => panic!("invalid value for frame type: {:?}", value),
        }
    }

    fn as_usize(&self) -> usize {
        *self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallocError {
    FreeWithAcquire,
    /// Frame allocator encoutered an unexpected frame state.
    /// Field 0 (usize): frame index
    /// Field 1 (FrameState): expected state
    /// Field 2 (FrameState): actual state
    ExpectedFrameState(usize, FrameState, FrameState),
    OutOfBoundsExists(usize),
    OutOfBoundsLock,
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
    memory_map: BitValueArray<'arr, FrameState>,
    memory: RwLock<[usize; FrameState::MASK + 1]>,
    non_usable_oob: core::cell::RefCell<alloc::collections::BTreeSet<usize>>,
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
        let falloc = Self {
            memory_map: BitValueArray::from_slice(
                core::ptr::slice_from_raw_parts_mut(
                    base_ptr,
                    BitValueArray::<FrameState>::section_length_hint(total_frames),
                )
                .as_mut()
                .unwrap(),
                total_frames,
            ),
            memory: RwLock::new(memory_counters),
            non_usable_oob: core::cell::RefCell::new(alloc::collections::BTreeSet::<usize>::new()),
        };

        falloc
            .acquire_frames(
                (base_ptr as usize) / 0x1000,
                Self::frame_count_hint(total_memory),
                FrameState::Reserved,
            )
            .expect("unexpectedly failed to reserve frame allocator frames");
        falloc.acquire_frame(0, FrameState::Reserved).unwrap();

        falloc
    }

    /* FREE / LOCK / RESERVE / STACK - SINGLE */

    /// Attempts to free a specific frame in the allocator.
    pub unsafe fn free_frame(&self, frame: Frame) -> Result<(), FallocError> {
        if self
            .memory_map
            .insert_eq(frame.index(), FrameState::Free, FrameState::Locked)
        {
            let mut mem_write = self.memory.write();
            mem_write[FrameState::Free.as_usize()] += 0x1000;
            mem_write[FrameState::Locked.as_usize()] -= 0x1000;

            Ok(())
        } else {
            Err(FallocError::ExpectedFrameState(
                frame.index(),
                FrameState::Locked,
                self.memory_map.get(frame.index()),
            ))
        }
    }

    pub unsafe fn acquire_frame(
        &self,
        index: usize,
        acq_state: FrameState,
    ) -> Result<Frame, FallocError> {
        match acq_state {
            // Track out-of-bounds Reserved frames.
            state if index >= self.memory_map.len() => {
                if state == FrameState::Reserved {
                    let mut non_usable_oob = self.non_usable_oob.borrow_mut();

                    if non_usable_oob.insert(index) {
                        Ok(Frame::from_index(index))
                    } else {
                        Err(FallocError::OutOfBoundsExists(index))
                    }
                } else {
                    Err(FallocError::OutOfBoundsLock)
                }
            }
            // Illegal to free with acquisition.
            FrameState::Free => Err(FallocError::FreeWithAcquire),
            // Acquire frame as locked if it is already free.
            FrameState::Locked
                if self
                    .memory_map
                    .insert_eq(index, acq_state, FrameState::Free) =>
            {
                let mut mem_write = self.memory.write();
                mem_write[FrameState::Free.as_usize()] -= 0x1000;
                mem_write[acq_state.as_usize()] += 0x1000;

                Ok(Frame::from_index(index))
            }
            // Failed to acquire frame as locked (incorrect existing frame type).
            FrameState::Locked => Err(FallocError::ExpectedFrameState(
                index,
                FrameState::Free,
                FrameState::Locked,
            )),
            // Acquire specific frame as non-usable (this will NEVER fail).
            FrameState::Reserved => {
                let old_state = self.memory_map.insert(index, FrameState::Reserved);
                if old_state != FrameState::Reserved {
                    self.memory.write()[old_state.as_usize()] -= 0x1000;
                }

                Ok(Frame::from_index(index))
            }
        }
    }

    pub unsafe fn unreserve_frame(&self, frame: Frame) -> Result<(), FallocError> {
        if self
            .memory_map
            .insert_eq(frame.index(), FrameState::Free, FrameState::Reserved)
        {
            let mut mem_write = self.memory.write();
            mem_write[FrameState::Free.as_usize()] += 0x1000;
            mem_write[FrameState::Reserved.as_usize()] -= 0x1000;

            Ok(())
        } else {
            Err(FallocError::ExpectedFrameState(
                frame.index(),
                FrameState::Reserved,
                self.memory_map.get(frame.index()),
            ))
        }
    }

    /* FREE / LOCK / RESERVE / STACK - ITER */

    /// Attempts to free many frames from an iterator.
    pub unsafe fn free_frames(
        &self,
        frames: impl Iterator<Item = Frame>,
    ) -> Result<(), FallocError> {
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
    ) -> Result<crate::memory::FrameIterator, FallocError> {
        let start_index = index;
        let end_index = index + count;
        for frame_index in start_index..end_index {
            if let Err(error) = self.acquire_frame(frame_index, acq_state) {
                return Err(error);
            }
        }

        Ok(crate::memory::FrameIterator::new(start_index..end_index))
    }

    /// Attempts to iterate the allocator's frames, and returns the first unallocated frame.
    pub fn autolock(&self) -> Option<Frame> {
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

    pub fn autolock_many(&self, count: usize) -> Option<FrameIterator> {
        let base_index = core::lazy::OnceCell::new();
        let mut current_run = 0;

        for (frame_index, frame_state) in self.memory_map.iter().enumerate() {
            if frame_state == FrameState::Free {
                current_run += 1;
            } else {
                current_run = 0;
            }

            if current_run == count {
                // Count to next frame index to ensure we cover only the currently checked indexes.
                base_index.set((frame_index + 1) - current_run).unwrap();
                break;
            }
        }

        base_index.get().map(|frame_index| {
            for index in *frame_index..(*frame_index + count) {
                self.memory_map.insert(index, FrameState::Locked);
            }

            unsafe { FrameIterator::new(*frame_index..(*frame_index + count)) }
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

    pub fn iter<'outer>(&'arr self) -> BitValueArrayIterator<'outer, 'arr, FrameState> {
        self.memory_map.iter()
    }

    #[cfg(debug_assertions)]
    pub fn debug_log_elements(&self) {
        self.memory_map.debug_log_elements();
    }
}
