use efi_boot::{MemoryDescriptor, MemoryType};
use x86_64::PhysAddr;

use crate::{structures::memory::PAGE_SIZE, BitArray};

pub struct PageFrameAllocator<'arr> {
    total_memory: usize,
    free_memory: usize,
    used_memory: usize,
    bitarray: BitArray<'arr>,
}

impl<'arr> PageFrameAllocator<'arr> {
    pub fn from_mmap(memory_map: &[MemoryDescriptor]) -> Self {
        let total_memory = memory_map
            .iter()
            .map(|descriptor| (descriptor.page_count as usize) * PAGE_SIZE)
            .sum();

        let bitarray_length = (total_memory / PAGE_SIZE / 8) + 1;
        let bitarray_pages = ((bitarray_length / PAGE_SIZE) + 1) as usize;
        debug!(
            "Minimum page frame allocator bit array length is {}.",
            bitarray_length
        );
        debug!(
            "Attempting to locate {} free pages for page frame allocator bit array.",
            bitarray_pages
        );

        let desctiptor = memory_map
            .iter()
            .find(|descriptor| {
                descriptor.ty == MemoryType::CONVENTIONAL
                    && descriptor.page_count as usize >= bitarray_pages
            })
            .expect("failed to find viable memory descriptor for bit array.");

        Self {
            total_memory,
            free_memory: total_memory,
            used_memory: 0,
            bitarray: BitArray::from_ptr(
                desctiptor.phys_start as *mut usize,
                bitarray_length as usize,
            ),
        }
    }

    pub fn free_page(&mut self, address: PhysAddr) {
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if self.bitarray.get_bit(index).unwrap() {
            self.bitarray.set_bit(index, false);
            self.free_memory += PAGE_SIZE;
            self.used_memory -= PAGE_SIZE;
        }
    }

    pub fn lock_page(&mut self, address: PhysAddr) {
        let index = (address.as_u64() as usize) / PAGE_SIZE;

        if !self.bitarray.get_bit(index).unwrap() {
            self.bitarray.set_bit(index, true);
            self.free_memory -= PAGE_SIZE;
            self.used_memory += PAGE_SIZE;
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
}
