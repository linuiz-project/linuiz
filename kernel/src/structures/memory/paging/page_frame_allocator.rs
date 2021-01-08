use efi_boot::{MemoryDescriptor, MemoryType};
use x86_64::PhysAddr;

use crate::{
    structures::memory::{Frame, PAGE_SIZE},
    BitArray,
};

pub struct PageFrameAllocator<'arr> {
    total_memory: usize,
    free_memory: usize,
    used_memory: usize,
    reserved_memory: usize,
    bitarray: BitArray<'arr>,
}

impl<'arr> PageFrameAllocator<'arr> {
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

        // lock pages this page frame allocator exists on
        unsafe {
            this.lock_pages(
                PhysAddr::new(descriptor.phys_start),
                bitarray_mem_size_pages,
            );
        }

        // reserve system pages
        for descriptor in memory_map {
            match descriptor.ty {
                MemoryType::LOADER_CODE
                | MemoryType::LOADER_DATA
                | MemoryType::BOOT_SERVICES_CODE
                | MemoryType::BOOT_SERVICES_DATA
                | MemoryType::CONVENTIONAL => {}
                _ => {
                    let phys_addr = PhysAddr::new(descriptor.phys_start);
                    debug!("Reserving pages at {:?}:\n{:#?}", phys_addr, descriptor);
                    unsafe { this.reserve_pages(phys_addr, descriptor.page_count as usize) }
                }
            }
        }
        info!(
            "{} KB of memory has been reserved by the system.",
            crate::structures::memory::to_kibibytes(this.reserved_memory)
        );

        this
    }

    pub unsafe fn free_pages(&mut self, address: PhysAddr, count: usize) {
        for index in 0..count {
            self.free_page(address + (index * PAGE_SIZE));
        }
    }

    pub unsafe fn free_page(&mut self, address: PhysAddr) {
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if self
            .bitarray
            .get_bit(index)
            .expect("failed to reserve page")
        {
            self.bitarray.set_bit(index, false);
            self.free_memory += PAGE_SIZE;
            self.used_memory -= PAGE_SIZE;
            trace!("Freed page: index {}, {:?}", index, address);
        }
    }

    pub unsafe fn lock_pages(&mut self, address: PhysAddr, count: usize) {
        for index in 0..count {
            self.lock_page(address + (index * PAGE_SIZE));
        }
    }

    pub unsafe fn lock_page(&mut self, address: PhysAddr) {
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if !self
            .bitarray
            .get_bit(index)
            .expect("failed to reserve page")
        {
            self.bitarray.set_bit(index, true);
            self.free_memory -= PAGE_SIZE;
            self.used_memory += PAGE_SIZE;
            trace!("Locked page: index {}, {:?}", index, address);
        }
    }

    pub(crate) unsafe fn unreserve_pages(&mut self, address: PhysAddr, count: usize) {
        for index in 0..count {
            self.unreserve_page(address + (index * PAGE_SIZE));
        }
    }

    pub(crate) unsafe fn unreserve_page(&mut self, address: PhysAddr) {
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if self
            .bitarray
            .get_bit(index)
            .expect("failed to reserve page")
        {
            self.bitarray.set_bit(index, false);
            self.free_memory += PAGE_SIZE;
            self.reserved_memory -= PAGE_SIZE;
            trace!("Unreserved page: index {}, {:?}", index, address);
        }
    }

    pub(crate) unsafe fn reserve_pages(&mut self, address: PhysAddr, count: usize) {
        for index in 0..count {
            self.reserve_page(address + (index * PAGE_SIZE));
        }
    }

    pub(crate) unsafe fn reserve_page(&mut self, address: PhysAddr) {
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if !self
            .bitarray
            .get_bit(index)
            .expect("failed to reserve page")
        {
            self.bitarray.set_bit(index, false);
            self.free_memory -= PAGE_SIZE;
            self.reserved_memory += PAGE_SIZE;
            trace!("Reserved page: index {}, {:?}", index, address);
        }
    }

    pub fn next_free(&mut self) -> Option<Frame> {
        match self.bitarray.iter().enumerate().find(|tuple| !tuple.1) {
            Some(result) => unsafe { Some(Frame::from_index(result.0 as u64)) },
            None => None,
        }
    }

    pub fn next_free_range(&mut self, count: usize) -> Option<PhysAddr> {
        for (index, locked) in self.bitarray.iter().enumerate() {
            if self
                .bitarray
                .iter()
                .skip(index)
                .take(count)
                .all(|locked| !locked)
            {
                for index0 in index..(index + count) {
                    self.bitarray.set_bit(index0, true);
                }

                return Some(PhysAddr::new((index * PAGE_SIZE) as u64));
            }
        }

        None
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
