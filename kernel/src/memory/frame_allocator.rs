use crate::memory::{is_uefi_reserved_memory_type, Frame, FrameIterator, FrameMap, FrameType};
use spin::RwLock;

struct FrameAllocatorMemory {
    total_memory: usize,
    free_memory: usize,
    used_memory: usize,
    reserved_memory: usize,
}

impl FrameAllocatorMemory {
    pub const fn new(total_memory: usize) -> Self {
        Self {
            total_memory,
            free_memory: total_memory,
            used_memory: 0,
            reserved_memory: 0,
        }
    }
}

pub struct FrameAllocator<'arr> {
    memory_map: FrameMap<'arr>,
    memory: RwLock<FrameAllocatorMemory>,
}

impl<'arr> FrameAllocator<'arr> {
    pub(super) fn from_mmap(uefi_memory_map: &[crate::memory::UEFIMemoryDescriptor]) -> Self {
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
        let len = total_memory / 0x1000;
        let size_bytes = FrameMap::size_hint_bytes(len);
        let page_count = (efi_boot::align_up(size_bytes, 0x1000) as u64) / 0x1000;
        debug!("Searching for memory descriptor which meets criteria:\n Pages: >= {}\n Bytes: >= {}\n Length: {}", page_count, size_bytes, len);
        let descriptor = uefi_memory_map
            .iter()
            .find(|descriptor| descriptor.page_count >= page_count)
            .expect("failed to find viable memory descriptor for memory map.");

        let mut this = Self {
            memory_map: unsafe {
                FrameMap::from_ptr(descriptor.phys_start.as_u64() as *mut usize, len)
            },
            memory: RwLock::new(FrameAllocatorMemory::new(total_memory)),
        };

        // reserve frames this page frame allocator exists on
        debug!(
            "Reserving frames for this allocator's memory map (total {} frames).",
            page_count
        );
        unsafe {
            let start_addr = descriptor.phys_start;
            let end_addr = start_addr + (page_count * 0x1000);
            this.reserve_frames(Frame::range_inclusive(
                start_addr.as_u64()..end_addr.as_u64(),
            ));
        }

        // reserve null frame
        unsafe { this.reserve_frame(&Frame::null()) };
        // reserve system frames
        for descriptor in uefi_memory_map
            .iter()
            .filter(|descriptor| is_uefi_reserved_memory_type(descriptor.ty))
        {
            trace!("Reserving frames for descriptor:\n{:#?}", descriptor);
            unsafe {
                this.reserve_frames(Frame::range_count(
                    descriptor.phys_start,
                    descriptor.page_count,
                ));
            };
        }

        info!(
            "{} KB of memory has been reserved by the system.",
            crate::memory::to_kibibytes(this.memory.read().reserved_memory)
        );

        this
    }

    pub fn total_memory(&self) -> usize {
        self.memory.read().total_memory
    }

    pub fn free_memory(&self) -> usize {
        self.memory.read().free_memory
    }

    pub fn locked_memory(&self) -> usize {
        self.memory.read().used_memory
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
            memory.used_memory -= 0x1000;
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
            memory.used_memory += 0x1000;
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
                let frame = Frame::from_index(index as u64);
                trace!("Locked next frame {}: {:?}", frame.index(), frame);

                return Some(frame);
            }
        }

        None
    }

    // todo get rid of this
    pub fn lock_next_count(&self, count: usize) -> Option<FrameIterator> {
        for mut index in 0..self.memory_map.len() {
            if self.memory_map.get(index) != FrameType::Unallocated {
                continue;
            } else {
                let mut all_unallocated = true;
                let high_bound = core::cmp::min(index + count, self.memory_map.len());

                for inner_index in (index + 1)..high_bound {
                    if self.memory_map.get(inner_index) != FrameType::Unallocated {
                        all_unallocated = false;
                        index = inner_index + 1;
                        break;
                    }
                }

                if all_unallocated && index >= (index + count) {
                    let high_index = index + count;
                    for inner_index in index..high_index {
                        self.memory_map.set(inner_index, FrameType::Allocated);
                    }

                    let low_addr = (index as u64) * 0x1000;
                    let high_addr = (high_index as u64) * 0x1000;
                    trace!("Many frames allocated from {} to {}", low_addr, high_addr);
                    return Some(Frame::range_inclusive(low_addr..high_addr));
                }
            }
        }

        None
    }
}
