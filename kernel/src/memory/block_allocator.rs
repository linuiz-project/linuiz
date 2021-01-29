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
    const REPR_PAGES_PER_SELFPAGE: usize = Self::REPR_BYTES_PER_SELFPAGE / 0x1000;

    const ALLOCATOR_BASE_PAGE: Page =
        unsafe { Page::from_addr(VirtAddr::new_unsafe((SYSTEM_SLICE_SIZE as u64) * 0xA)) };
    const ALLOCATOR_CAPACITY: usize = SYSTEM_SLICE_SIZE;

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

    pub fn malloc(&self, size: usize) -> &mut [u8] {
        unsafe { &mut *slice_from_raw_parts_mut(self.ralloc(size), size) }
    }

    fn ralloc(&self, size: usize) -> *mut u8 {
        let size_in_blocks = efi_boot::align_up(size, Self::BLOCK_SIZE) / Self::BLOCK_SIZE;
        trace!(
            "Allocation requested: {} bytes => {} blocks",
            size,
            size_in_blocks
        );

        while size_in_blocks >= self.blocks_count() {
            self.grow_once();
        }

        let map_read = self.map.upgradeable_read();
        let max_map_index = (map_read.len() * 8) - size_in_blocks;
        let mut block_index = 0;
        let mut current_run = 0;

        'byte: for byte in (0..max_map_index).map(|map_index| map_read[map_index]) {
            for shift_bit in (0..8).map(|shift| 1 << shift) {
                if (byte & shift_bit) == 0 {
                    current_run += 1;
                } else {
                    current_run = 0;
                }

                block_index += 1;

                if current_run == size_in_blocks {
                    break 'byte;
                }
            }
        }

        if current_run == size_in_blocks {
            let start_block_index = block_index - current_run;
            trace!(
                "Allocating section: blocks {}..{}",
                start_block_index,
                start_block_index + size_in_blocks
            );

            let mut map_write = map_read.upgrade();
            for map_index in (start_block_index / 8)..((block_index + 7) / 8) {
                if current_run >= 8 {
                    map_write[map_index] = u8::MAX;
                    current_run -= 8;
                } else if current_run > 0 {
                    map_write[map_index] |= (0..current_run).map(|shift| 1 << shift).sum::<u8>();
                }
            }

            (self.base_page.addr() + (start_block_index * Self::BLOCK_SIZE)).as_mut_ptr()
        } else {
            panic!("failed to allocate")
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
        } else {
            let self_pages_count = map.len() / 0x1000;
            // map the next frame for the allocator map
            self.addressor.lock().get_mut().unwrap().map(
                &Self::ALLOCATOR_BASE_PAGE.offset(self_pages_count),
                unsafe { &global_lock_next().unwrap() },
            );

            // update the map slice to reflect new size
            let mut map = map.upgrade();
            *map = unsafe { &mut *slice_from_raw_parts_mut(map.as_mut_ptr(), map.len() + 0x1000) };

            // allocate and map the pages that the new slice area covers
            for page in self
                .base_page
                .offset(self_pages_count * Self::REPR_PAGES_PER_SELFPAGE)
                .iter_count(Self::REPR_PAGES_PER_SELFPAGE)
            {
                self.addressor
                    .lock()
                    .get_mut()
                    .unwrap()
                    .map(&page, unsafe { &global_lock_next().unwrap() });
            }
        }
    }
}

unsafe impl core::alloc::GlobalAlloc for BlockAllocator<'_> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.ralloc(layout.size())
    }

    unsafe fn dealloc(&self, _: *mut u8, __: core::alloc::Layout) {}
}
