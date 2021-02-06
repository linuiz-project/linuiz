use crate::{
    memory::{global_lock_next, paging::VirtualAddressor, Page},
    SYSTEM_SLICE_SIZE,
};
use core::{lazy::OnceCell, ptr::slice_from_raw_parts_mut};
use spin::{Mutex, RwLock};
use x86_64::VirtAddr;

pub struct BlockAllocator<'map> {
    addressor: Mutex<OnceCell<VirtualAddressor>>,
    map: RwLock<&'map mut [u8]>,
    base_page: Page,
}

impl BlockAllocator<'_> {
    const BLOCK_SIZE: usize = 16;
    const BITS_PER_SELFPAGE: usize = 0x1000 * 8 /* bits per byte */;
    const REPR_BYTES_PER_SELFPAGE: usize = Self::BITS_PER_SELFPAGE * Self::BLOCK_SIZE;
    const BLOCKS_PER_SELFPAGE: usize = Self::REPR_BYTES_PER_SELFPAGE / 0x1000;

    const ALLOCATOR_BASE_PAGE: Page =
        unsafe { Page::from_addr(VirtAddr::new_unsafe((SYSTEM_SLICE_SIZE as u64) * 0xA)) };
    const ALLOCATOR_CAPACITY: usize = SYSTEM_SLICE_SIZE;

    const MASK_MAP: [u8; 8] = [
        0b1, 0b11, 0b111, 0b1111, 0b11111, 0b111111, 0b11111111, 0b11111111,
    ];

    pub const fn new(base_page: Page) -> Self {
        Self {
            addressor: Mutex::new(OnceCell::new()),
            map: RwLock::new(&mut [0u8; 0]),
            base_page,
        }
    }

    pub unsafe fn set_addressor(&self, virtual_addressor: VirtualAddressor) {
        if virtual_addressor.is_mapped(Self::ALLOCATOR_BASE_PAGE.addr()) {
            panic!("allocator already exists for this virtual addressor (or allocator memory zone has been otherwise mapped)");
        } else if let Err(_) = self.addressor.lock().set(virtual_addressor) {
            panic!("addressor has already been set for allocator");
        } else {
            *self.map.write() =
                &mut *slice_from_raw_parts_mut(Self::ALLOCATOR_BASE_PAGE.addr().as_mut_ptr(), 0);
        }
    }

    pub fn blocks_count(&self) -> usize {
        self.map.read().len() * 8
    }

    fn raw_alloc(&self, size: usize) -> *mut u8 {
        trace!("Allocation requested: {} bytes", size);

        let size_in_blocks = (size + (Self::BLOCK_SIZE - 1)) / Self::BLOCK_SIZE;
        let initial_block_count = self.blocks_count();
        if size_in_blocks > initial_block_count {
            let required_growth = ((size_in_blocks - initial_block_count)
                + (Self::BLOCKS_PER_SELFPAGE - 1))
                / Self::BLOCKS_PER_SELFPAGE;

            (0..required_growth).for_each(|_| self.grow_once());
        }

        let map_read = self.map.upgradeable_read();
        let max_map_index = (map_read.len() * 8) - size_in_blocks;
        let mut block_index = 0;
        let mut current_run = 0;

        'outer: for byte in (0..max_map_index).map(|map_index| map_read[map_index]) {
            for bit in (0..8).map(|shift| (byte & (1 << shift)) == 0) {
                if bit {
                    current_run += 1;
                } else {
                    current_run = 0;
                }

                block_index += 1;

                if current_run == size_in_blocks {
                    break 'outer;
                }
            }
        }

        if current_run == size_in_blocks {
            let start_block_index = block_index - current_run;
            let end_block_index = block_index;
            block_index = start_block_index;
            trace!(
                "Allocating blocks: {}..{}",
                start_block_index,
                end_block_index
            );

            let start_map_index = start_block_index / 8;
            for (map_index, byte) in map_read
                .upgrade()
                .iter_mut()
                .enumerate()
                .skip(start_map_index)
                .take(((end_block_index + 7) / 8) - start_map_index)
            {
                let traversed_blocks = map_index * 8;
                let start_byte_bit = block_index - traversed_blocks;
                let end_byte_bit = core::cmp::min(8, end_block_index - traversed_blocks);
                let total_bits = end_byte_bit - start_byte_bit;
                let value = Self::MASK_MAP[total_bits - 1] << start_byte_bit;

                debug_assert_eq!(
                    *byte & value,
                    0,
                    "attempting to allocate blocks that are already allocated"
                );

                *byte |= value;
                block_index += total_bits;
            }

            (self.base_page.addr() + (start_block_index * Self::BLOCK_SIZE)).as_mut_ptr()
        } else {
            panic!("out of memory!")
        }
    }

    fn raw_dealloc(&self, ptr: *mut u8, size: usize) {
        let start_block_index =
            ((ptr as usize) - (self.base_page.addr().as_u64() as usize)) / Self::BLOCK_SIZE;
        let end_block_index =
            start_block_index + ((size + (Self::BLOCK_SIZE - 1)) / Self::BLOCK_SIZE);
        let mut block_index = start_block_index;
        trace!(
            "Deallocating blocks: {}..{}",
            start_block_index,
            end_block_index
        );

        let start_map_index = start_block_index / 8;
        for (map_index, byte) in self
            .map
            .write()
            .iter_mut()
            .enumerate()
            .skip(start_map_index)
            .take(((end_block_index + 7) / 8) - start_map_index)
        {
            let traversed_blocks = map_index * 8;
            let start_byte_bit = block_index - traversed_blocks;
            let end_byte_bit = core::cmp::min(8, end_block_index - traversed_blocks);
            let total_bits = end_byte_bit - start_byte_bit;
            let value = Self::MASK_MAP[total_bits - 1] << start_byte_bit;

            debug_assert_eq!(
                *byte & value,
                value,
                "attempting to deallocate blocks that are aren't allocated"
            );

            *byte ^= value;
            block_index += total_bits;
        }
    }

    pub fn grow_once(&self) {
        trace!(
            "Allocator map too small: {} blocks; growing by one page.",
            self.blocks_count()
        );

        let map = self.map.upgradeable_read();
        if map.len() >= Self::ALLOCATOR_CAPACITY {
            panic!("allocator has reached maximum memory");
        } else if let Some(addressor) = self.addressor.lock().get_mut() {
            let self_pages_count = map.len() / 0x1000;
            // map the next frame for the allocator map
            addressor.map(
                &Self::ALLOCATOR_BASE_PAGE.offset(self_pages_count),
                unsafe { &global_lock_next().unwrap() },
            );

            // update the map slice to reflect new size
            let mut map = map.upgrade();
            *map = unsafe { &mut *slice_from_raw_parts_mut(map.as_mut_ptr(), map.len() + 0x1000) };

            // allocate and map the pages that the new slice area covers
            for (index, page) in self
                .base_page
                .offset(self_pages_count * Self::BLOCKS_PER_SELFPAGE)
                .iter_count(Self::BLOCKS_PER_SELFPAGE)
                .enumerate()
            {
                trace!("Clearing page after growing (iter {}): {:?}", index, page);
                addressor.map(&page, unsafe { &global_lock_next().unwrap() });
            }

            trace!("Successfully grew allocator map.");
        } else {
            panic!("addressor has not been set for allocator.");
        }
    }
}

unsafe impl core::alloc::GlobalAlloc for BlockAllocator<'_> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.raw_alloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.raw_dealloc(ptr, layout.size());
    }
}
