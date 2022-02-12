use core::{alloc::Layout, mem::size_of, num::NonZeroUsize};
use lib::{
    align_up_div,
    memory::{
        get_page_manager,
        malloc::{Alloc, AllocError, MemoryAllocator},
        Page, PageAttributes, FRAME_MANAGER,
    },
    Address, Physical,
};
use spin::{RwLock, RwLockWriteGuard};

/// Represents one page worth of memory blocks (i.e. 4096 bytes in blocks).
#[repr(transparent)]
#[derive(Clone)]
struct BlockPage(u64);

impl BlockPage {
    /// How many bits/block indexes in section primitive.
    const BLOCKS_PER: usize = size_of::<u64>() * 8;

    /// Whether the block page is empty.
    pub const fn is_empty(&self) -> bool {
        self.0 == u64::MIN
    }

    /// Whether the block page is full.
    pub const fn is_full(&self) -> bool {
        self.0 == u64::MAX
    }

    /// Unset all of the block page's blocks.
    pub const fn set_empty(&mut self) {
        self.0 = u64::MIN;
    }

    /// Set all of the block page's blocks.
    pub const fn set_full(&mut self) {
        self.0 = u64::MAX;
    }

    pub const fn value(&self) -> &u64 {
        &self.0
    }

    pub const fn value_mut(&mut self) -> &mut u64 {
        &mut self.0
    }
}

impl core::fmt::Debug for BlockPage {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("BlockPage")
            .field(&format_args!("0b{:b}", self.0))
            .finish()
    }
}

/// Allocator utilizing blocks of memory, in size of 64 bytes per block, to
///  easily and efficiently allocate.
pub struct SLOB<'map> {
    map: RwLock<&'map mut [BlockPage]>,
}

impl<'map> SLOB<'map> {
    /// The size of an allocator block.
    pub const BLOCK_SIZE: usize = 0x1000 / BlockPage::BLOCKS_PER;

    #[allow(const_item_mutation)]
    pub fn new() -> Self {
        const EMPTY: [BlockPage; 0] = [];

        Self {
            map: RwLock::new(&mut EMPTY),
        }
    }

    /// Calculates the bit count and mask for a given set of block page parameters.
    fn calculate_bit_fields(
        map_index: usize,
        cur_block_index: usize,
        end_block_index: usize,
    ) -> (usize, u64) {
        let floor_blocks_index = map_index * BlockPage::BLOCKS_PER;
        let ceil_blocks_index = floor_blocks_index + BlockPage::BLOCKS_PER;
        let mask_bit_offset = cur_block_index - floor_blocks_index;
        let mask_bit_count = usize::min(ceil_blocks_index, end_block_index) - cur_block_index;

        (
            mask_bit_count,
            ((1 as u64) << mask_bit_count).wrapping_sub(1) << mask_bit_offset,
        )
    }

    pub unsafe fn reserve_page(&self, page: &Page) -> Result<(), AllocError> {
        let mut map_write = self.map.write();

        if map_write.len() <= page.index() {
            if let Err(error) = self.grow(
                usize::max(
                    // page.index() + 1 to facilitate page indexes that are power-of-two, i.e.:
                    //  page.index() =      2048
                    //  map_write.len() =   1024
                    //  Map grows to facilitate 2048 block pages, and page index 2048 is still out of bounds.
                    (page.index() + 1) - map_write.len(),
                    1,
                ) * BlockPage::BLOCKS_PER,
                &mut map_write,
            ) {
                return Err(error);
            }
        }

        if map_write[page.index()].is_empty() {
            map_write[page.index()].set_full();

            Ok(())
        } else {
            Err(AllocError::TryReserveNonEmptyPage)
        }
    }

    fn grow(
        &self,
        required_blocks: usize,
        map_write: &mut RwLockWriteGuard<&mut [BlockPage]>,
    ) -> Result<(), AllocError> {
        assert!(required_blocks > 0, "calls to grow must be nonzero");

        trace!(
            "Allocator map requires growth: {} blocks required.",
            required_blocks
        );

        // Current length of our map, in indexes.
        let cur_map_len = map_write.len();
        // Required length of our map, in indexes.
        let req_map_len = (map_write.len()
            + lib::align_up_div(required_blocks, BlockPage::BLOCKS_PER))
        .next_power_of_two();
        // Current page count of our map (i.e. how many pages the slice requires)
        let cur_map_pages = lib::align_up_div(cur_map_len * size_of::<BlockPage>(), 0x1000);
        // Required page count of our map.
        let req_map_pages = lib::align_up_div(req_map_len * size_of::<BlockPage>(), 0x1000);

        trace!(
            "Growth parameters: len {} => {}, pages {} => {}",
            cur_map_len,
            req_map_len,
            cur_map_pages,
            req_map_pages
        );

        // Attempt to find a run of already-mapped pages within our allocator
        // that can contain our required slice length.
        let mut current_run = 0;
        let start_index = core::lazy::OnceCell::new();
        for (index, block_page) in map_write.iter().enumerate() {
            if block_page.is_empty() {
                current_run += 1;

                if current_run == req_map_pages {
                    start_index.set(index - (current_run - 1)).unwrap();
                    break;
                }
            } else {
                current_run = 0;
            }
        }

        let cur_map_page = Page::from_index((map_write.as_ptr() as usize) / 0x1000);
        let new_map_page = Page::from_index(*start_index.get_or_init(|| {
            // When the map is zero-sized, this allows us to skip the first page in our
            // allocations (in order to keep the 0th page as null & unmapped).
            if cur_map_len == 0 {
                cur_map_len + 1
            } else {
                cur_map_len
            }
        }));

        trace!("Copy mapping current map to new pages.");
        for page_offset in 0..cur_map_pages {
            get_page_manager()
                .copy_by_map(
                    &cur_map_page.forward(page_offset).unwrap(),
                    &new_map_page.forward(page_offset).unwrap(),
                    None,
                )
                .unwrap();
        }

        trace!("Allocating and mapping remaining pages of map.");
        for page_offset in cur_map_pages..req_map_pages {
            let mut new_page = new_map_page.forward(page_offset).unwrap();

            get_page_manager().auto_map(&new_page, PageAttributes::DATA_BITS);
            // Clear the newly allocated map page.
            unsafe { new_page.mem_clear() };
        }

        // Point to new map.
        **map_write = unsafe {
            core::slice::from_raw_parts_mut(
                new_map_page.as_mut_ptr(),
                lib::align_up(req_map_len, 0x1000 / size_of::<BlockPage>()),
            )
        };

        map_write
            .iter_mut()
            .skip(new_map_page.index())
            .take(req_map_pages)
            .for_each(|block_page| block_page.set_full());
        map_write
            .iter_mut()
            .skip(cur_map_page.index())
            .take(cur_map_pages)
            .for_each(|block_page| block_page.set_empty());

        Ok(())
    }
}

impl MemoryAllocator for SLOB<'_> {
    fn alloc(&self, size: usize, align: Option<NonZeroUsize>) -> Result<Alloc<u8>, AllocError> {
        let align = align
            .unwrap_or(unsafe { NonZeroUsize::new_unchecked(1) })
            .get();
        if !align.is_power_of_two() {
            return Err(AllocError::InvalidAlignment(align));
        }

        let align_mask = usize::max(align / Self::BLOCK_SIZE, 1) - 1;
        let size_in_blocks = lib::align_up_div(size, Self::BLOCK_SIZE);
        let mut map_write = self.map.write();

        let end_map_index;
        let mut block_index;
        let mut current_run;

        'outer: loop {
            block_index = 0;
            current_run = 0;

            for (map_index, block_page) in map_write.iter().enumerate() {
                if block_page.is_full() {
                    current_run = 0;
                    block_index += BlockPage::BLOCKS_PER;
                } else {
                    for bit_shift in 0..BlockPage::BLOCKS_PER {
                        if (block_page.value() & (1 << bit_shift)) > 0 {
                            current_run = 0;
                        } else if current_run > 0 || (bit_shift & align_mask) == 0 {
                            current_run += 1;

                            if current_run == size_in_blocks {
                                end_map_index = map_index + 1;
                                break 'outer;
                            }
                        }

                        block_index += 1;
                    }
                }
            }

            if let Err(alloc_err) = self.grow(size_in_blocks, &mut map_write) {
                return Err(alloc_err);
            }
        }

        let end_block_index = block_index + 1;
        block_index -= current_run - 1;
        let start_block_index = block_index;
        let start_map_index = start_block_index / BlockPage::BLOCKS_PER;

        for map_index in start_map_index..end_map_index {
            let block_page = &mut map_write[map_index];
            let was_empty = block_page.is_empty();

            let block_index_floor = map_index * BlockPage::BLOCKS_PER;
            let low_offset = block_index - block_index_floor;
            let remaining_blocks_in_slice = usize::min(
                end_block_index - block_index,
                (block_index_floor + BlockPage::BLOCKS_PER) - block_index,
            );
            let mask_bits = (1 as u64)
                .checked_shl(remaining_blocks_in_slice as u32)
                .unwrap_or(u64::MAX)
                .wrapping_sub(1);

            *block_page.value_mut() |= mask_bits << low_offset;
            block_index += remaining_blocks_in_slice;

            if was_empty {
                get_page_manager()
                    .auto_map(&Page::from_index(map_index), PageAttributes::DATA_BITS);
            }
        }

        Ok(unsafe {
            Alloc::new(
                (start_block_index * Self::BLOCK_SIZE) as *mut _,
                size_in_blocks * Self::BLOCK_SIZE,
            )
        })
    }

    // TODO this should not allocate contiguous frames
    fn alloc_pages(&self, count: usize) -> Result<(Address<Physical>, Alloc<u8>), AllocError> {
        let mut map_write = self.map.write();
        let frame_index = match FRAME_MANAGER.lock_next_many(count) {
            Ok(frame_index) => frame_index,
            Err(falloc_err) => {
                return Err(AllocError::FallocError(falloc_err));
            }
        };

        let mut start_index = 0;
        'outer: loop {
            let mut current_run = 0;

            for (map_index, block_page) in map_write.iter_mut().enumerate().skip(start_index) {
                if !block_page.is_empty() {
                    current_run = 0;
                    start_index = map_index + 1;
                } else {
                    current_run += 1;

                    if current_run == count {
                        break 'outer;
                    }
                }
            }

            if let Err(alloc_err) = self.grow(count * BlockPage::BLOCKS_PER, &mut map_write) {
                return Err(alloc_err);
            }
        }

        for offset in 0..count {
            let page_index = start_index + offset;
            let frame_index = frame_index + offset;

            map_write[page_index].set_full();
            get_page_manager()
                .map(
                    &Page::from_index(page_index),
                    frame_index,
                    None,
                    PageAttributes::DATA_BITS,
                )
                .unwrap();
        }

        Ok((Address::<Physical>::new(frame_index * 0x1000), unsafe {
            Alloc::new((start_index * 0x1000) as *mut _, count * 0x1000)
        }))
    }

    fn alloc_against(&self, frame_index: usize, count: usize) -> Result<Alloc<u8>, AllocError> {
        let mut map_write = self.map.write();
        let mut start_index = 0;
        'outer: loop {
            let mut current_run = 0;

            for (map_index, block_page) in map_write.iter_mut().enumerate().skip(start_index) {
                if !block_page.is_empty() {
                    current_run = 0;
                    start_index = map_index + 1;
                } else {
                    current_run += 1;

                    if current_run == count {
                        break 'outer;
                    }
                }
            }

            if let Err(alloc_err) = self.grow(count * BlockPage::BLOCKS_PER, &mut map_write) {
                return Err(alloc_err);
            }
        }

        for offset in 0..count {
            let page_index = start_index + offset;
            let frame_index = frame_index + offset;

            map_write[page_index].set_full();
            get_page_manager()
                .map(
                    &Page::from_index(page_index),
                    frame_index,
                    None,
                    PageAttributes::DATA_BITS,
                )
                .unwrap();
        }

        Ok(unsafe { Alloc::new((start_index * 0x1000) as *mut _, count * 0x1000) })
    }

    fn alloc_identity(&self, frame_index: usize, count: usize) -> Result<Alloc<u8>, AllocError> {
        let mut map_write = self.map.write();

        if map_write.len() < (frame_index + count) {
            self.grow(
                (frame_index + count) * BlockPage::BLOCKS_PER,
                &mut map_write,
            )
            .unwrap();
        }

        for page_index in frame_index..(frame_index + count) {
            if map_write[page_index].is_empty() {
                map_write[page_index].set_full();
                get_page_manager()
                    .identity_map(&Page::from_index(page_index), PageAttributes::DATA_BITS)
                    .unwrap();
            } else {
                for page_index in frame_index..page_index {
                    map_write[page_index].set_empty();
                    get_page_manager()
                        .unmap(&Page::from_index(page_index), false)
                        .unwrap();
                }

                return Err(AllocError::IdentityMappingOverlaps);
            }
        }

        Ok(unsafe { Alloc::new((frame_index * 0x1000) as *mut _, count * 0x1000) })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let start_block_index = (ptr as usize) / Self::BLOCK_SIZE;
        let end_block_index = start_block_index + align_up_div(layout.size(), Self::BLOCK_SIZE);
        let mut block_index = start_block_index;
        trace!(
            "Deallocation requested: {}..{}",
            start_block_index,
            end_block_index
        );

        let start_map_index = start_block_index / BlockPage::BLOCKS_PER;
        let end_map_index = align_up_div(end_block_index, BlockPage::BLOCKS_PER);
        let mut map_write = self.map.write();
        for map_index in start_map_index..end_map_index {
            let (had_bits, has_bits) = {
                let block_page = &mut map_write[map_index];

                let had_bits = !block_page.is_empty();

                let (bit_count, bit_mask) =
                    Self::calculate_bit_fields(map_index, block_index, end_block_index);
                assert_eq!(
                    *block_page.value() & bit_mask,
                    bit_mask,
                    "attempting to deallocate blocks that are already deallocated"
                );

                *block_page.value_mut() ^= bit_mask;
                block_index += bit_count;

                (had_bits, !block_page.is_empty())
            };

            if had_bits && !has_bits {
                // FIXME:
                // This is wildly unsafe. There's no way to validate—without *only* going through
                // the Rust global allocator—that the given page allocated a frame and wasn't a direct allocation
                // of some sort.
                get_page_manager()
                    .unmap(&Page::from_index(map_index), true)
                    .unwrap();
            }
        }
    }

    fn get_page_state(&self, page_index: usize) -> Option<bool> {
        self.map
            .read()
            .get(page_index)
            .map(|block_page| !block_page.is_empty())
    }
}
