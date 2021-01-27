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
    map: RwLock<&'map mut [u8]>,
    base_page: Page,
}

impl<'vaddr, 'map> BlockAllocator<'vaddr, 'map> {
    const ALLOCATOR_BASE_PAGE: Page =
        unsafe { Page::from_addr(VirtAddr::new_unsafe((SYSTEM_SLICE_SIZE as u64) * 0xA)) };
    const ALLOCATOR_CAPACITY: usize = SYSTEM_SLICE_SIZE;
    const BLOCK_SIZE: usize = 16;
    const PAGES_PER_SLAB: usize = 8 * Self::BLOCK_SIZE;

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

    pub fn alloc(&self, size: usize) -> &mut [u8] {
        // if size >= the amount of bytes represented by each u8 in our map
        //  — (which is 8 bits * BLOCK_SIZE)
        if size >= (Self::BLOCK_SIZE * 8) {
            // we can search and check if indexes are 0
            let index_count = efi_boot::align_up(size, Self::BLOCK_SIZE * 8);
            let map_read = self.map.upgradeable_read();

            for mut index in (0..map_read.len()).filter(|index| map_read[*index] > 0) {
                if map_read
                    .iter()
                    .skip(index)
                    .take(index_count)
                    .all(|value| *value == 0)
                {
                    map_read
                        .upgrade()
                        .iter_mut()
                        .skip(index)
                        .take(index_count)
                        .for_each(|value| *value = u8::MAX);

                    return unsafe {
                        &mut *slice_from_raw_parts_mut(self.base_page.offset((index * Self::PAGES_PER_SLAB)), len)
                    };
                } else {
                    index += index_count;
                }
            }
        } else {
            // we have to bit twiddle each bit to find adequate allocation
        }

        &mut [0u8; 0]
    }

    pub fn grow_once(&self) {
        let map_read = self.map.upgradeable_read();

        if map_read.len() >= Self::ALLOCATOR_CAPACITY {
            panic!("allocator has reached maximum memory");
        } else {
            // map the next frame for the allocator map
            self.virtual_addressor.map(
                &Self::ALLOCATOR_BASE_PAGE.offset(map_read.len() / 0x1000),
                unsafe { &global_lock_next().unwrap() },
            );

            // update the map slice to reflect new size
            let mut map_write = map_read.upgrade();
            *map_write = unsafe {
                &mut *slice_from_raw_parts_mut(map_write.as_mut_ptr(), map_write.len() + 0x1000)
            };

            // allocate and map the pages that the new slice area covers
            for page in self
                .base_page
                .offset((map_read.len() / 0x1000) * Self::PAGES_PER_SLAB)
                .iter_count(Self::PAGES_PER_SLAB)
            {
                self.virtual_addressor
                    .map(&page, unsafe { &global_lock_next().unwrap() });
            }
        }
    }
}
