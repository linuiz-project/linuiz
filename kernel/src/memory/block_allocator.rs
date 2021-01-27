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
    const BLOCKS_PER_PAGE: usize = 0x1000 / Self::BLOCK_SIZE;

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

    fn grow_once(&self) {
        let map_read = self.map.upgradeable_read();

        if map_read.len() >= Self::ALLOCATOR_CAPACITY {
            panic!("out of memory");
        } else {
            let frame = unsafe { global_lock_next() }.expect("failed to allocate frame for self");
            let current_pages = map_read.len() / 0x1000;
            let page = Self::ALLOCATOR_BASE_PAGE.offset(current_pages);
            self.virtual_addressor.map(&page, &frame);

            let mut map_write = map_read.upgrade();

            *map_write = unsafe {
                &mut *slice_from_raw_parts_mut(map_write.as_mut_ptr(), map_write.len() + 0x1000)
            };
        }
    }
}
