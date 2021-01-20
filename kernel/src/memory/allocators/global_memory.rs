use crate::{
    memory::{is_reservable_memory_type, Frame, FrameIterator},
    BitArray,
};
use efi_boot::MemoryDescriptor;
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};

pub struct FrameAllocator<'arr> {
    total_memory: usize,
    free_memory: usize,
    used_memory: usize,
    reserved_memory: usize,
    bitarray: Mutex<BitArray<'arr>>,
}

impl<'arr> FrameAllocator<'arr> {
    pub(super) const fn uninit() -> Self {
        Self {
            total_memory: 0,
            free_memory: 0,
            used_memory: 0,
            reserved_memory: 0,
            bitarray: Mutex::new(BitArray::empty()),
        }
    }

    pub(super) fn from_mmap(memory_map: &[MemoryDescriptor]) -> Self {
        let last_descriptor = memory_map
            .iter()
            .max_by_key(|descriptor| descriptor.phys_start)
            .expect("no descriptor with max value");
        let total_memory =
            (last_descriptor.phys_start + (last_descriptor.page_count * 0x1000)) as usize;
        debug!(
            "Page frame allocator will represent {} MB ({} bytes) of system memory.",
            crate::memory::to_mibibytes(total_memory),
            total_memory
        );

        // allocate the bit array
        let bitarray_bits = total_memory / 0x1000;
        let bitarray_page_count: u64 =
            ((bitarray_bits / crate::bitarray::SECTION_BITS_COUNT) + 1) as u64;
        let descriptor = memory_map
            .iter()
            .find(|descriptor| descriptor.page_count >= bitarray_page_count)
            .expect("failed to find viable memory descriptor for bit array.");
        debug!(
            "Identified acceptable descriptor for bitarray:\n{:#?}",
            descriptor
        );

        let bitarray =
            BitArray::from_ptr(descriptor.phys_start as *mut usize, bitarray_bits as usize);
        debug!(
            "Successfully initialized bitarray with a length of {}.",
            bitarray.bit_count()
        );

        let mut this = Self {
            total_memory,
            free_memory: total_memory,
            used_memory: 0,
            reserved_memory: 0,
            bitarray: Mutex::new(bitarray),
        };

        // reserve frames this page frame allocator exists on
        debug!(
            "Reserving frames for this allocator's bitarray (total {} frames).",
            bitarray_page_count
        );
        unsafe {
            let start_addr = descriptor.phys_start;
            let end_addr = start_addr + (bitarray_page_count * 0x1000);
            this.reserve_frames(Frame::range(start_addr..end_addr));
        }

        // reserve null frame
        unsafe { this.reserve_frame(Frame::from_index(0)) };
        // reserve system frames
        for descriptor in memory_map
            .iter()
            .filter(|descriptor| is_reservable_memory_type(descriptor.ty))
        {
            let start_addr = descriptor.phys_start;
            let end_addr = start_addr + (descriptor.page_count * 0x1000);
            trace!(
                "Reserving {} frames at {:?}:\n{:#?}",
                descriptor.page_count,
                PhysAddr::new(start_addr),
                descriptor
            );
            unsafe { this.reserve_frames(Frame::range(start_addr..end_addr)) };
        }
        info!(
            "{} KB of memory has been reserved by the system.",
            crate::memory::to_kibibytes(this.reserved_memory)
        );

        this
    }

    pub fn total_memory(&self) -> usize {
        self.total_memory
    }

    pub fn free_memory(&self) -> usize {
        self.free_memory
    }

    pub fn locked_memory(&self) -> usize {
        self.used_memory
    }

    pub fn reserved_memory(&self) -> usize {
        self.reserved_memory
    }

    pub fn physical_mapping_addr(&self) -> VirtAddr {
        VirtAddr::new((0x1000000000000 - self.total_memory) as u64)
    }

    /* SINGLE OPS */
    pub unsafe fn free_frame(&mut self, frame: Frame) {
        let mut bitarray = self.bitarray.lock();
        let index = frame.index() as usize;

        if bitarray.get_bit(index).expect("failed to free frame") {
            bitarray.set_bit(index, false);
            self.free_memory += 0x1000;
            self.used_memory -= 0x1000;
            trace!("Freed frame {}: {:?}", index, frame);
        }
    }

    pub unsafe fn lock_frame(&mut self, frame: Frame) {
        let mut bitarray = self.bitarray.lock();
        let index = frame.index() as usize;

        if bitarray.get_bit(index).expect("failed to lock frame") {
            bitarray.set_bit(index, true);
            self.free_memory -= 0x1000;
            self.used_memory += 0x1000;
            trace!("Locked frame {}: {:?}", index, frame);
        }
    }

    pub(crate) unsafe fn reserve_frame(&mut self, frame: Frame) {
        let mut bitarray = self.bitarray.lock();
        let index = frame.index() as usize;

        if !bitarray.get_bit(index).expect("failed to reserve frame") {
            bitarray.set_bit(index, true);
            self.free_memory -= 0x1000;
            self.reserved_memory += 0x1000;
            trace!("Reserved frame {}: {:?}", index, frame);
        }
    }

    /* MANY OPS */
    pub unsafe fn free_frames(&mut self, frames: FrameIterator) {
        for frame in frames {
            self.free_frame(frame);
        }
    }

    pub unsafe fn lock_frames(&mut self, frames: FrameIterator) {
        for frame in frames {
            self.lock_frame(frame);
        }
    }

    pub(crate) unsafe fn reserve_frames(&mut self, frames: FrameIterator) {
        for frame in frames {
            self.reserve_frame(frame);
        }
    }

    pub fn lock_next(&mut self) -> Option<Frame> {
        let mut bitarray = self.bitarray.lock();

        match bitarray.iter().enumerate().find(|tuple| !tuple.1) {
            Some(tuple) => {
                trace!(
                    "Located frame {}, which is unallocated and safe for allocation.",
                    tuple.0
                );
                bitarray.set_bit(tuple.0, true);
                Some(Frame::from_index(tuple.0 as u64))
            }
            None => None,
        }
    }

    pub fn lock_next_count(&mut self, count: usize) -> Option<FrameIterator> {
        let mut bitarray = self.bitarray.lock();
        let mut index = 0;

        while let Some(locked) = bitarray.get_bit(index) {
            if locked {
                index += 1;
            } else {
                if let Some(tuple) = bitarray
                    .get_bits(index..(index + count))
                    .enumerate()
                    .find(|tuple| tuple.1)
                {
                    index += tuple.0;
                } else {
                    let high_index = index + count;
                    for inner_index in index..high_index {
                        bitarray.set_bit(inner_index, true).unwrap();
                    }

                    let low_addr = (index as u64) * 0x1000;
                    let high_addr = (high_index as u64) * 0x1000;
                    trace!("Many frames allocated from {} to {}", low_addr, high_addr);
                    return Some(Frame::range(low_addr..high_addr));
                }
            }
        }

        None
    }
}

// #[global_allocator]
static mut GLOBAL_MEMORY: FrameAllocator<'static> = FrameAllocator::uninit();

pub unsafe fn init_global_memory(memory_map: &[MemoryDescriptor]) {
    GLOBAL_MEMORY = FrameAllocator::from_mmap(memory_map);
}

pub fn global_memory<C, R>(callback: C) -> R
where
    C: Fn(&FrameAllocator) -> R,
{
    callback(unsafe { &GLOBAL_MEMORY })
}

pub fn global_memory_mut<C, R>(mut callback: C) -> R
where
    C: FnMut(&mut FrameAllocator) -> R,
{
    callback(unsafe { &mut GLOBAL_MEMORY })
}

pub fn total_memory_iter() -> core::ops::Range<usize> {
    0..global_memory(|allocator| allocator.total_memory())
}
