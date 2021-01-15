use crate::{
    structures::memory::{Frame, FrameIterator, PAGE_SIZE},
    BitArray,
};
use core::alloc::{GlobalAlloc, Layout};
use efi_boot::{align_up, MemoryDescriptor, MemoryType};
use spin::Mutex;
use x86_64::PhysAddr;

pub struct FrameAllocator<'arr> {
    total_memory: usize,
    free_memory: usize,
    used_memory: usize,
    reserved_memory: usize,
    bitarray: Mutex<BitArray<'arr>>,
}

impl<'arr> FrameAllocator<'arr> {
    pub(self) const fn uninit() -> Self {
        Self {
            total_memory: 0,
            free_memory: 0,
            used_memory: 0,
            reserved_memory: 0,
            bitarray: Mutex::new(BitArray::empty()),
        }
    }

    fn from_mmap(memory_map: &[MemoryDescriptor]) -> Self {
        let last_descriptor = memory_map
            .iter()
            .max_by_key(|descriptor| descriptor.phys_start)
            .expect("no descriptor with max value");
        let total_memory =
            last_descriptor.phys_start as usize + (last_descriptor.page_count as usize * PAGE_SIZE);
        debug!(
            "Page frame allocator will represent {} MB ({} bytes) of system memory.",
            crate::structures::memory::to_mibibytes(total_memory),
            total_memory
        );

        // allocate the bit array
        let bitarray_bits = total_memory / PAGE_SIZE;
        let bitarray_mem_size_pages = (bitarray_bits / crate::bitarray::SECTION_BITS_COUNT) + 1;
        debug!(
            "Attempting to create a frame allocator with {} frames.",
            bitarray_bits
        );
        let descriptor = memory_map
            .iter()
            .max_by_key(|descriptor| {
                if descriptor.ty == MemoryType::CONVENTIONAL {
                    descriptor.page_count
                } else {
                    0
                }
            })
            .expect("failed to find viable memory descriptor for bit array.");

        debug!(
            "Identified acceptable descriptor for bit array:\n{:#?}",
            descriptor
        );
        let bitarray =
            BitArray::from_ptr(descriptor.phys_start as *mut usize, bitarray_bits as usize);
        debug!(
            "BitArray initialized with a length of {}.",
            bitarray.bit_count()
        );

        // crate the page frame allocator
        let mut this = Self {
            total_memory,
            free_memory: total_memory,
            used_memory: 0,
            reserved_memory: 0,
            bitarray: Mutex::new(bitarray),
        };

        // lock frames this page frame allocator exists on
        debug!(
            "Reserving frames for this allocator's bitarray (total {} frames).",
            bitarray_mem_size_pages
        );
        unsafe {
            this.reserve_frames(
                PhysAddr::new(descriptor.phys_start),
                bitarray_mem_size_pages,
            );
        }

        // reserve null frame
        unsafe { this.reserve_frame(PhysAddr::zero()) };
        // reserve system frames
        for descriptor in memory_map {
            match descriptor.ty {
                MemoryType::LOADER_CODE
                | MemoryType::LOADER_DATA
                | MemoryType::BOOT_SERVICES_CODE
                | MemoryType::BOOT_SERVICES_DATA
                | MemoryType::CONVENTIONAL => {}
                _ => {
                    let phys_addr = PhysAddr::new(descriptor.phys_start);
                    debug!(
                        "Reserving {} frames at {:?}:\n{:#?}",
                        descriptor.page_count, phys_addr, descriptor
                    );
                    unsafe { this.reserve_frames(phys_addr, descriptor.page_count as usize) }
                }
            }
        }
        info!(
            "{} KB of memory has been reserved by the system.",
            crate::structures::memory::to_kibibytes(this.reserved_memory)
        );

        this
    }

    pub fn total_memory(&self) -> usize {
        self.total_memory
    }

    pub fn free_memory(&self) -> usize {
        self.free_memory
    }

    pub fn used_memory(&self) -> usize {
        self.used_memory
    }

    pub fn reserved_memory(&self) -> usize {
        self.reserved_memory
    }

    /* SINGLE OPS */
    pub unsafe fn deallocate_frame(&mut self, address: PhysAddr) {
        let mut bitarray = self.bitarray.lock();
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if bitarray.get_bit(index).expect("failed to free frame") {
            bitarray.set_bit(index, false);
            self.free_memory += PAGE_SIZE;
            self.used_memory -= PAGE_SIZE;
            trace!("Freed frame {}: {:?}", index, address);
        }
    }

    // todo return frames maybe? with identity
    pub unsafe fn allocate_frame(&mut self, address: PhysAddr) {
        let mut bitarray = self.bitarray.lock();
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if bitarray.get_bit(index).expect("failed to lock frame") {
            bitarray.set_bit(index, true);
            self.free_memory -= PAGE_SIZE;
            self.used_memory += PAGE_SIZE;
            trace!("Locked frame {}: {:?}", index, address);
        }
    }

    pub(crate) unsafe fn reserve_frame(&mut self, address: PhysAddr) {
        let mut bitarray = self.bitarray.lock();
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if !bitarray.get_bit(index).expect("failed to reserve frame") {
            bitarray.set_bit(index, true);
            self.free_memory -= PAGE_SIZE;
            self.reserved_memory += PAGE_SIZE;
            trace!("Reserved frame {}: {:?}", index, address);
        }
    }

    /* MANY OPS */
    pub unsafe fn deallocate_frames(&mut self, address: PhysAddr, count: usize) {
        for index in 0..count {
            self.deallocate_frame(address + (index * PAGE_SIZE));
        }
    }

    pub unsafe fn allocate_frames(&mut self, address: PhysAddr, count: usize) {
        for index in 0..count {
            self.allocate_frame(address + (index * PAGE_SIZE));
        }
    }

    pub(crate) unsafe fn reserve_frames(&mut self, address: PhysAddr, count: usize) {
        for index in 0..count {
            self.reserve_frame(address + (index * PAGE_SIZE));
        }
    }

    pub fn alloc_next(&mut self) -> Option<Frame> {
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

    pub fn alloc_many(&mut self, count: usize) -> Option<FrameIterator> {
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

unsafe impl GlobalAlloc for FrameAllocator<'static> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let frame_count = align_up(layout.size(), layout.align());

        global_allocator_mut(|allocator| {
            if let Some(mut frame_iter) = allocator.alloc_many(frame_count) {
                frame_iter.next().unwrap().addr().as_u64() as *mut u8
            } else {
                core::ptr::null_mut::<u8>()
            }
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        global_allocator_mut(|allocator| {
            allocator.deallocate_frames(
                PhysAddr::new(ptr as u64),
                align_up(layout.size(), layout.align()),
            );
        })
    }
}

#[global_allocator]
static mut GLOBAL_ALLOCATOR: FrameAllocator<'static> = FrameAllocator::uninit();

pub unsafe fn init_global_allocator(memory_map: &[MemoryDescriptor]) {
    GLOBAL_ALLOCATOR = FrameAllocator::from_mmap(memory_map);
}

pub fn global_allocator<F, R>(callback: F) -> R
where
    F: Fn(&FrameAllocator) -> R,
{
    callback(unsafe { &GLOBAL_ALLOCATOR })
}

pub fn global_allocator_mut<F, R>(mut callback: F) -> R
where
    F: FnMut(&mut FrameAllocator) -> R,
{
    callback(unsafe { &mut GLOBAL_ALLOCATOR })
}
