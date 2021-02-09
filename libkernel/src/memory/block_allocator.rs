use crate::{
    memory::{global_lock_next, paging::VirtualAddressor, Page},
    SYSTEM_SLICE_SIZE,
};
use alloc::vec::Vec;
use core::lazy::OnceCell;
use spin::{Mutex, RwLock};
use x86_64::VirtAddr;

pub struct BlockAllocator {
    addressor: Mutex<OnceCell<VirtualAddressor>>,
    map: RwLock<Vec<u8>>,
    alloc_page: Page,
}

impl BlockAllocator {
    const BLOCK_SIZE: usize = 16;
    const BLOCKS_PER_SELFPAGE: usize = Self::BLOCK_SIZE * 8;

    const ALLOCATOR_BASE: Page =
        Page::from_addr(VirtAddr::new_truncate((SYSTEM_SLICE_SIZE as u64) * 0xA));
    const ALLOCATOR_CAPACITY: usize = SYSTEM_SLICE_SIZE;

    const MASK_MAP: [u8; 8] = [
        0b1, 0b11, 0b111, 0b1111, 0b11111, 0b111111, 0b11111111, 0b11111111,
    ];

    pub const fn new(base_page: Page) -> Self {
        Self {
            addressor: Mutex::new(OnceCell::new()),
            map: RwLock::new(Vec::new()),
            alloc_page: base_page,
        }
    }

    pub unsafe fn set_addressor(&self, virtual_addressor: VirtualAddressor) {
        if virtual_addressor.is_mapped(Self::ALLOCATOR_BASE.addr()) {
            panic!("allocator already exists for this virtual addressor (or allocator memory zone has been otherwise mapped)");
        } else if let Err(_) = self.addressor.lock().set(virtual_addressor) {
            panic!("addressor has already been set for allocator");
        } else {
            *self.map.write() = Vec::from_raw_parts(
                Self::ALLOCATOR_BASE.mut_ptr() as *mut u8,
                0,
                Self::ALLOCATOR_CAPACITY,
            );
        }
    }

    pub fn blocks_count(&self) -> usize {
        self.map.read().len() * 8
    }

    fn raw_alloc(&self, size: usize) -> *mut u8 {
        trace!("Allocation requested: {} bytes", size);

        let size_in_blocks = (size + (Self::BLOCK_SIZE - 1)) / Self::BLOCK_SIZE;
        let (mut block_index, mut current_run);

        while {
            block_index = 0;
            current_run = 0;
            let map_read = self.map.read();

            for bit in map_read
                .iter()
                .take((map_read.len() * 8) - size_in_blocks)
                .map(|byte| *byte)
                .flat_map(|byte| (0..8).map(move |shift| (byte & (1 << shift)) == 0))
            {
                if bit {
                    current_run += 1;
                } else {
                    current_run = 0;
                }

                block_index += 1;

                if current_run == size_in_blocks {
                    break;
                }
            }

            // grow while we can't accomodate allocation
            current_run < size_in_blocks
        } {
            self.grow_once();
        }

        let start_block_index = block_index - current_run;
        let end_block_index = block_index;
        block_index = start_block_index;
        trace!(
            "Allocating blocks: {}..{}",
            start_block_index,
            end_block_index
        );

        let start_map_index = start_block_index / 8;
        for (traversed_blocks, byte) in self
            .map
            .write()
            .iter_mut()
            .enumerate()
            .skip(start_map_index)
            .take(((end_block_index + 7) / 8) - start_map_index)
            .map(|(map_index, byte)| (map_index * 8, byte))
        {
            let start_byte_bits = block_index - traversed_blocks;
            let total_bits =
                core::cmp::min(8, end_block_index - traversed_blocks) - start_byte_bits;
            let bits_mask = Self::MASK_MAP[total_bits - 1] << start_byte_bits;

            debug_assert_eq!(
                *byte & bits_mask,
                0,
                "attempting to allocate blocks that are already allocated"
            );

            *byte |= bits_mask;
            block_index += total_bits;
        }

        (self.alloc_page.addr() + (start_block_index * Self::BLOCK_SIZE)).as_mut_ptr()
    }

    fn raw_dealloc(&self, ptr: *mut u8, size: usize) {
        let start_block_index =
            ((ptr as usize) - (self.alloc_page.addr().as_u64() as usize)) / Self::BLOCK_SIZE;
        let end_block_index =
            start_block_index + ((size + (Self::BLOCK_SIZE - 1)) / Self::BLOCK_SIZE);
        let mut block_index = start_block_index;
        trace!(
            "Deallocating blocks: {}..{}",
            start_block_index,
            end_block_index
        );

        let start_map_index = start_block_index / 8;
        for (traversed_blocks, byte) in self
            .map
            .write()
            .iter_mut()
            .enumerate()
            .skip(start_map_index)
            .take(((end_block_index + 7) / 8) - start_map_index)
            .map(|(map_index, byte)| (map_index * 8, byte))
        {
            let start_byte_bit = block_index - traversed_blocks;
            let total_bits = core::cmp::min(8, end_block_index - traversed_blocks) - start_byte_bit;
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

        let map_read = self.map.upgradeable_read();
        let map_read_len = map_read.len();
        if map_read_len >= Self::ALLOCATOR_CAPACITY {
            panic!("allocator has reached maximum memory");
        } else if let Some(addressor) = self.addressor.lock().get_mut() {
            let self_pages_count = map_read_len / 0x1000;
            // map the next frame for the allocator map
            addressor.map(&Self::ALLOCATOR_BASE.offset(self_pages_count), unsafe {
                &global_lock_next().unwrap()
            });

            // update the map slice to reflect new size
            map_read.upgrade().resize(map_read_len + 0x1000, 0);

            // allocate and map the pages that the new slice area covers
            for page in self
                .alloc_page
                .offset(self_pages_count * Self::BLOCKS_PER_SELFPAGE)
                .iter_count(Self::BLOCKS_PER_SELFPAGE)
            {
                trace!("Clearing page after growing: {:?}", page);
                addressor.map(&page, unsafe { &global_lock_next().unwrap() });
            }

            trace!("Successfully grew allocator map.");
        } else {
            panic!("addressor has not been set for allocator.");
        }
    }
}

unsafe impl core::alloc::GlobalAlloc for BlockAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.raw_alloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.raw_dealloc(ptr, layout.size());
    }
}
