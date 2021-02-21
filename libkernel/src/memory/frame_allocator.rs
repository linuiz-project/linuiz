use crate::{
    memory::{Frame, FrameIterator},
    BitValue, RwBitArray,
};
use spin::RwLock;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Unallocated = 0,
    Allocated,
    Reserved,
    Stack,
}

impl BitValue for FrameType {
    const BIT_WIDTH: usize = 0x2;
    const MASK: usize = (Self::BIT_WIDTH << 1) - 1;

    fn from_usize(value: usize) -> Self {
        match value {
            0 => FrameType::Unallocated,
            1 => FrameType::Allocated,
            2 => FrameType::Reserved,
            3 => FrameType::Stack,
            _ => panic!("invalid value, must be 0..=3"),
        }
    }

    fn as_usize(&self) -> usize {
        *self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAllocationError {
    NotAllocated,
    NotUnallocated,
}

struct FrameAllocatorMemory {
    total_memory: usize,
    free_memory: usize,
    locked_memory: usize,
    reserved_memory: usize,
}

pub struct FrameAllocator<'arr> {
    memory_map: RwBitArray<'arr, FrameType>,
    memory: RwLock<FrameAllocatorMemory>,
}

impl<'arr> FrameAllocator<'arr> {
    pub fn frame_count_hint(total_memory: usize) -> usize {
        assert_eq!(
            total_memory % 0x1000,
            0,
            "system memory should be page-aligned"
        );

        let frame_count = total_memory / 0x1000;
        crate::align_up_div(
            // each index of a RwBitArray is a `usize`, so multiply the index count with the `size_of` a `usize`
            RwBitArray::<FrameType>::length_hint(frame_count) * core::mem::size_of::<usize>(),
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
            memory: RwLock::new(FrameAllocatorMemory {
                total_memory,
                free_memory: total_memory,
                locked_memory: 0,
                reserved_memory: 0,
            }),
        };

        let base_frame = Frame::from_addr(x86_64::PhysAddr::new(base_ptr as u64));
        let bitarray_frames = Frame::range_count(base_frame, Self::frame_count_hint(total_memory));

        // reserve null frame
        this.reserve_frame(&Frame::null()).unwrap();
        // reserve bitarray frames
        this.reserve_frames(bitarray_frames);

        this
    }

    pub fn total_memory(&self) -> usize {
        self.memory.read().total_memory
    }

    pub fn free_memory(&self) -> usize {
        self.memory.read().free_memory
    }

    pub fn locked_memory(&self) -> usize {
        self.memory.read().locked_memory
    }

    pub fn reserved_memory(&self) -> usize {
        self.memory.read().reserved_memory
    }

    /* SINGLE OPS */
    pub unsafe fn free_frame(&self, frame: &Frame) -> Result<(), FrameAllocationError> {
        if self
            .memory_map
            .set_eq(frame.index(), FrameType::Unallocated, FrameType::Allocated)
        {
            let mut memory = self.memory.write();
            memory.free_memory += 0x1000;
            memory.locked_memory -= 0x1000;

            trace!("Freed frame: {:?}", frame);
            Ok(())
        } else {
            Err(FrameAllocationError::NotAllocated)
        }
    }

    pub unsafe fn lock_frame(&self, frame: &Frame) -> Result<(), FrameAllocationError> {
        if self
            .memory_map
            .set_eq(frame.index(), FrameType::Allocated, FrameType::Unallocated)
        {
            let mut memory = self.memory.write();
            memory.free_memory -= 0x1000;
            memory.locked_memory += 0x1000;

            trace!("Locked frame: {:?}", frame);
            Ok(())
        } else {
            Err(FrameAllocationError::NotUnallocated)
        }
    }

    pub fn lock_next(&self) -> Option<Frame> {
        self.memory_map
            .set_eq_next(FrameType::Allocated, FrameType::Unallocated)
            .map(|index| {
                let frame = Frame::from_index(index);
                let mut memory = self.memory.write();
                memory.free_memory -= 0x1000;
                memory.locked_memory += 0x1000;

                trace!("Locked next free frame: {:?}", frame);
                frame
            })
    }

    pub unsafe fn reserve_frame(&self, frame: &Frame) -> Result<(), FrameAllocationError> {
        if self
            .memory_map
            .set_eq(frame.index(), FrameType::Reserved, FrameType::Unallocated)
        {
            let mut memory = self.memory.write();
            memory.free_memory -= 0x1000;
            memory.reserved_memory += 0x1000;

            trace!("Reserved frame: {:?}", frame);
            Ok(())
        } else {
            Err(FrameAllocationError::NotUnallocated)
        }
    }

    pub unsafe fn reserve_stack(&self, frames: FrameIterator) -> Result<(), FrameAllocationError> {
        for frame in frames {
            if self
                .memory_map
                .set_eq(frame.index(), FrameType::Stack, FrameType::Unallocated)
            {
                let mut memory = self.memory.write();
                memory.free_memory -= 0x1000;
                memory.reserved_memory += 0x1000;

                trace!("Reserved stack frame: {:?}", frame);
            } else {
                return Err(FrameAllocationError::NotUnallocated);
            }
        }

        Ok(())
    }

    /* MANY OPS */
    pub unsafe fn free_frames(&self, frames: FrameIterator) {
        for frame in frames {
            self.free_frame(&frame).expect("failed to free frame");
        }
    }

    pub unsafe fn lock_frames(&self, frames: FrameIterator) {
        for frame in frames {
            self.lock_frame(&frame).expect("failed to lock frame");
        }
    }

    // TODO return result on all frameas
    pub unsafe fn reserve_frames(&self, frames: FrameIterator) {
        for frame in frames {
            self.reserve_frame(&frame).expect("failed to reserve frame");
        }
    }

    pub fn iter_callback<F>(&self, mut callback: F)
    where
        F: FnMut(usize, FrameType),
    {
        for index in 0..self.memory_map.len() {
            callback(index, self.memory_map.get(index));
        }
    }
}
