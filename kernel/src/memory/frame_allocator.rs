use crate::{
    memory::{is_uefi_reserved_memory_type, Frame, FrameIterator},
    BitArray, BitValue,
};
use spin::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Unallocated = 0,
    Allocated,
    Reserved,
    Corrupted,
}

impl BitValue for FrameType {
    const BIT_WIDTH: usize = 0x2;
    const MASK: usize = (Self::BIT_WIDTH << 1) - 1;

    fn from_usize(value: usize) -> Self {
        match value {
            0 => FrameType::Unallocated,
            1 => FrameType::Allocated,
            2 => FrameType::Reserved,
            3 => FrameType::Corrupted,
            _ => panic!("invalid cast"),
        }
    }

    fn as_usize(&self) -> usize {
        match self {
            FrameType::Unallocated => 0,
            FrameType::Allocated => 1,
            FrameType::Reserved => 2,
            FrameType::Corrupted => 3,
        }
    }
}

struct FrameAllocatorMemory {
    total_memory: usize,
    free_memory: usize,
    locked_memory: usize,
    reserved_memory: usize,
}

pub struct FrameAllocator<'arr> {
    memory_map: BitArray<'arr, FrameType>,
    memory: RwLock<FrameAllocatorMemory>,
}

impl<'arr> FrameAllocator<'arr> {
    pub(super) fn from_mmap(
        uefi_memory_map: &[crate::memory::UEFIMemoryDescriptor],
    ) -> (FrameIterator, Self) {
        let last_descriptor = uefi_memory_map
            .iter()
            .max_by_key(|descriptor| descriptor.phys_start)
            .expect("no descriptor with max value");
        let total_memory =
            (last_descriptor.phys_start.as_u64() + (last_descriptor.page_count * 0x1000)) as usize;
        debug!(
            "Page frame allocator will represent {} MB ({} bytes) of system memory.",
            crate::memory::to_mibibytes(total_memory),
            total_memory
        );

        // allocate the memory map
        let element_count = total_memory / 0x1000;
        let memory_size = (element_count * FrameType::BIT_WIDTH) / 8;
        let memory_pages = efi_boot::align_up(memory_size, 0x1000) / 0x1000;
        debug!("Searching for memory descriptor which meets criteria:\n Pages (Memory): {}\n Bytes (Memory): >= {}\n Pages (Represented): >= {}", memory_pages, memory_size, element_count);
        let descriptor = uefi_memory_map
            .iter()
            .find(|descriptor| descriptor.page_count >= (memory_pages as u64))
            .expect("failed to find viable memory descriptor for memory map.");
        debug!("Located usable memory descriptor:\n{:#?}", descriptor);

        let mut this = Self {
            memory_map: BitArray::from_slice(unsafe {
                &mut *core::ptr::slice_from_raw_parts_mut(
                    descriptor.phys_start.as_u64() as *mut _,
                    BitArray::<FrameType>::length_hint(element_count),
                )
            }),
            memory: RwLock::new(FrameAllocatorMemory {
                total_memory,
                free_memory: total_memory,
                locked_memory: 0,
                reserved_memory: 0,
            }),
        };

        unsafe {
            // reserve null frame
            this.reserve_frame(&Frame::null());
            // reserve bitarray frames
            this.reserve_frames(Frame::range_count(descriptor.phys_start, memory_pages));
            // reserve system & kernel frames
            for descriptor in uefi_memory_map
                .iter()
                .filter(|descriptor| is_uefi_reserved_memory_type(descriptor.ty))
            {
                this.reserve_frames(Frame::range_count(
                    descriptor.phys_start,
                    descriptor.page_count as usize,
                ));
            }
        }

        info!(
            "{} KB of memory has been reserved by the system.",
            crate::memory::to_kibibytes(this.memory.read().reserved_memory)
        );

        (
            Frame::range_count(descriptor.phys_start, memory_pages),
            this,
        )
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
    pub unsafe fn free_frame(&self, frame: &Frame) {
        if self.memory_map.set_eq(
            frame.index() as usize,
            FrameType::Unallocated,
            FrameType::Allocated,
        ) {
            let mut memory = self.memory.write();
            memory.free_memory += 0x1000;
            memory.locked_memory -= 0x1000;
            trace!("Freed frame {}: {:?}", frame.index(), frame);
        } else {
            panic!("attempted to reserve a non-free frame: {:?}", frame);
        }
    }

    pub unsafe fn lock_frame(&self, frame: &Frame) {
        if self.memory_map.set_eq(
            frame.index() as usize,
            FrameType::Allocated,
            FrameType::Unallocated,
        ) {
            let mut memory = self.memory.write();
            memory.free_memory -= 0x1000;
            memory.locked_memory += 0x1000;
            trace!("Locked frame {}: {:?}", frame.index(), frame);
        } else {
            panic!("attempted to reserve a non-free frame: {:?}", frame);
        }
    }

    pub(crate) unsafe fn reserve_frame(&self, frame: &Frame) {
        if self.memory_map.set_eq(
            frame.index() as usize,
            FrameType::Reserved,
            FrameType::Unallocated,
        ) {
            let mut memory = self.memory.write();
            memory.free_memory -= 0x1000;
            memory.reserved_memory += 0x1000;
            trace!("Reserved frame {}: {:?}", frame.index(), frame);
        } else {
            panic!("attempted to reserve a non-free frame: {:?}", frame);
        }
    }
    /* MANY OPS */
    pub unsafe fn free_frames(&self, frames: FrameIterator) {
        for frame in frames {
            self.free_frame(&frame);
        }
    }

    pub unsafe fn lock_frames(&self, frames: FrameIterator) {
        for frame in frames {
            self.lock_frame(&frame);
        }
    }

    pub(crate) unsafe fn reserve_frames(&mut self, frames: FrameIterator) {
        for frame in frames {
            self.reserve_frame(&frame);
        }
    }

    pub fn lock_next(&self) -> Option<Frame> {
        for index in 0..self.memory_map.len() {
            if self
                .memory_map
                .set_eq(index, FrameType::Allocated, FrameType::Unallocated)
            {
                let frame = Frame::from_index(index);
                trace!("Locked next frame {}: {:?}", frame.index(), frame);

                return Some(frame);
            }
        }

        None
    }
}
