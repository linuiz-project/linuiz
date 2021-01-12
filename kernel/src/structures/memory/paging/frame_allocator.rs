use crate::{
    structures::memory::{Frame, PAGE_SIZE},
    BitArray,
};
use efi_boot::{MemoryDescriptor, MemoryType};
use x86_64::PhysAddr;

pub struct FrameAllocator<'arr> {
    total_memory: usize,
    free_memory: usize,
    used_memory: usize,
    reserved_memory: usize,
    bitarray: BitArray<'arr>,
}

impl<'arr> FrameAllocator<'arr> {
    pub fn from_mmap(memory_map: &[MemoryDescriptor]) -> Self {
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
            bitarray,
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

    /* SINGLE OPS */
    pub unsafe fn deallocate_frame(&mut self, address: PhysAddr) {
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if self.bitarray.get_bit(index).expect("failed to free frame") {
            self.bitarray.set_bit(index, false);
            self.free_memory += PAGE_SIZE;
            self.used_memory -= PAGE_SIZE;
            trace!("Freed frame {}: {:?}", index, address);
        }
    }

    // todo return frames maybe? with identity
    pub unsafe fn allocate_frame(&mut self, address: PhysAddr) {
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if !self.bitarray.get_bit(index).expect("failed to lock frame") {
            self.bitarray.set_bit(index, true);
            self.free_memory -= PAGE_SIZE;
            self.used_memory += PAGE_SIZE;
            trace!("Locked frame {}: {:?}", index, address);
        }
    }

    pub(crate) unsafe fn reserve_frame(&mut self, address: PhysAddr) {
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if !self
            .bitarray
            .get_bit(index)
            .expect("failed to reserve frame")
        {
            self.bitarray.set_bit(index, true);
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

    pub fn allocate_next(&mut self) -> Option<Frame> {
        match self.bitarray.iter().enumerate().find(|tuple| !tuple.1) {
            Some(tuple) => {
                trace!(
                    "Located frame {}, which is unallocated and safe for allocation.",
                    tuple.0
                );
                self.bitarray.set_bit(tuple.0, true);
                Some(Frame::from_index(tuple.0 as u64))
            }
            None => None,
        }
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
}
