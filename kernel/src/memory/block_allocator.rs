use crate::{
    memory::{paging::VirtualAddressorCell, Page},
    SYSTEM_SLICE_SIZE,
};
use core::ptr::slice_from_raw_parts_mut;
use spin::RwLock;
use x86_64::VirtAddr;

use super::global_lock_next;

pub struct BlockAllocator<'vaddr, 'map> {
    virtual_addressor: &'vaddr VirtualAddressorCell,
    /// - Each bit represents BLOCK_SIZE worth of bytes
    /// -
    map: RwLock<&'map mut [u8]>,
    base_page: Page,
}

impl<'vaddr, 'map> BlockAllocator<'vaddr, 'map> {
    // FACTS:
    // Bits per self-page: 4096 * (8 bits) = 32768 bits
    //  - Each bit represents one block
    // Represented bytes per self-page: 32768 * (16 byte block size) = 524288 bytes
    // Represented pages per self-page: 524288 / 4096 = 128

    const BLOCK_SIZE: usize = 16;
    const BITS_PER_SELFPAGE: usize = 0x1000 * 8 /* bits per byte */;
    const REPR_BYTES_PER_SELFPAGE: usize = Self::BITS_PER_SELFPAGE * Self::BLOCK_SIZE;
    const REPR_PAGES_PER_SELFPAGE: usize = Self::REPR_BYTES_PER_SELFPAGE / 0x1000;
    const REPR_BYTES_PER_MAP_INDEX: usize = Self::BLOCK_SIZE * 8;

    const ALLOCATOR_BASE_PAGE: Page =
        unsafe { Page::from_addr(VirtAddr::new_unsafe((SYSTEM_SLICE_SIZE as u64) * 0xA)) };
    const ALLOCATOR_CAPACITY: usize = SYSTEM_SLICE_SIZE;

    pub fn new(base_page: Page, virtual_addressor: &'static VirtualAddressorCell) -> Self {
        if virtual_addressor.is_mapped(Self::ALLOCATOR_BASE_PAGE.addr()) {
            panic!("allocator already exists for this virtual addressor (or allocator memory zone has been otherwise mapped)");
        } else {
            Self {
                virtual_addressor,
                // todo raw vec
                map: RwLock::new(unsafe {
                    &mut *slice_from_raw_parts_mut(Self::ALLOCATOR_BASE_PAGE.addr().as_mut_ptr(), 0)
                }),
                base_page,
            }
        }
    }

    pub fn blocks_count(&self) -> usize {
        self.map().read().len() * 8
    }

    pub fn alloc(&self, size: usize) -> &mut [u8] {
        debug!("Attempting to allocate a section of {} bytes.", size);

        let size_in_blocks = efi_boot::align_up(size, Self::BLOCK_SIZE) / Self::BLOCK_SIZE;

        while size_in_blocks >= self.blocks_count() {
            self.grow_once();
        }

        let map_read = self.map.upgradeable_read();
        let max_map_index = map_read.len() - size_in_blocks;
        let mut current_run = 0;
        'map: for map_index in 0..max_map_index {
            let byte = &mut map_read[map_index];

            if byte == u8::MAX {
                current_run = 0;
            } else {
                'byte: for shift in 0..8 {
                    let shift_bit = 1 << shift;
                    let bit = byte & shift_bit;

                    if bit > 0 {
                        current_run += 1;

                        if current_run == size_in_blocks {
                            break 'map;
                        }
                    } else {
                        current_run = 0;
                        continue;
                    }
                }
            }
        }

        &mut [0u8; 0]
    }

    pub fn grow_once(&self) {
        let map = self.map.upgradeable_read();

        if map.len() >= Self::ALLOCATOR_CAPACITY {
            panic!("allocator has reached maximum memory");
        } else {
            // map the next frame for the allocator map
            self.virtual_addressor.map(
                &Self::ALLOCATOR_BASE_PAGE.offset(map.len() / 0x1000),
                unsafe { &global_lock_next().unwrap() },
            );

            // update the map slice to reflect new size
            let mut map = map.upgrade();
            *map = unsafe { &mut *slice_from_raw_parts_mut(map.as_mut_ptr(), map.len() + 0x1000) };

            // allocate and map the pages that the new slice area covers
            for page in self
                .base_page
                .offset((map.len() / 0x1000) * Self::REPR_PAGES_PER_SELFPAGE)
                .iter_count(Self::REPR_PAGES_PER_SELFPAGE)
            {
                self.virtual_addressor
                    .map(&page, unsafe { &global_lock_next().unwrap() });
            }
        }
    }
}
