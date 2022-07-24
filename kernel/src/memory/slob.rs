use core::{alloc::Layout, mem::size_of};
use liblz::{
    align_up_div,
    memory::{Page, PageAttributes},
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

pub enum AllocError {
    OutOfMemory,
}

/// Allocator utilizing blocks of memory, in size of 64 bytes per block, to
///  easily and efficiently allocate.
pub struct SLOB<'map> {
    table: RwLock<&'map mut [BlockPage]>,
}

impl<'map> SLOB<'map> {
    /// The size of an allocator block.
    pub const BLOCK_SIZE: usize = 0x1000 / BlockPage::BLOCKS_PER;

    pub unsafe fn new() -> Self {
        let initial_alloc_table_page = Page::from_index(1);
        let alloc_table_len = 0x1000 / core::mem::size_of::<BlockPage>();
        let global_pmgr = liblz::memory::global_pmgr();

        // Map all of the pages in the allocation table.
        for page_offset in 0..(alloc_table_len /* we don't map null page */ - 1) {
            global_pmgr.auto_map(
                &initial_alloc_table_page
                    .forward_checked(page_offset)
                    .unwrap(),
                PageAttributes::DATA,
            );
        }

        let alloc_table = core::slice::from_raw_parts_mut(
            initial_alloc_table_page.as_mut_ptr::<BlockPage>(),
            alloc_table_len,
        );
        // Always fill the 'null' page.
        alloc_table[0].set_full();
        // Additionally, fill the allocator table's page.
        alloc_table[initial_alloc_table_page.index()].set_full();

        Self {
            table: RwLock::new(alloc_table),
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

    fn grow(
        &self,
        required_blocks: core::num::NonZeroUsize,
        table_write: &mut RwLockWriteGuard<&mut [BlockPage]>,
    ) -> Result<(), AllocError> {
        // Current length of our map, in indexes.
        let cur_table_len = table_write.len();
        // Required length of our map, in indexes.
        let req_table_len = (table_write.len()
            + liblz::align_up_div(required_blocks.get(), BlockPage::BLOCKS_PER))
        .next_power_of_two();
        // Current page count of our map (i.e. how many pages the slice requires)
        let cur_table_page_count =
            liblz::align_up_div(cur_table_len * size_of::<BlockPage>(), 0x1000);
        // Required page count of our map.
        let req_table_page_count =
            liblz::align_up_div(req_table_len * size_of::<BlockPage>(), 0x1000);

        if (req_table_len * 0x1000) >= 0x746A52880000 {
            return Err(AllocError::OutOfMemory);
        }

        let page_manager = liblz::memory::global_pmgr();

        // Attempt to find a run of already-mapped pages within our allocator that can contain
        // the required slice length.
        let mut current_run = 0;
        let start_index = core::cell::OnceCell::new();
        for (index, block_page) in table_write.iter().enumerate() {
            if block_page.is_empty() {
                current_run += 1;

                if current_run == req_table_page_count {
                    start_index.set(index - (current_run - 1)).unwrap();
                    break;
                }
            } else {
                current_run = 0;
            }
        }

        let cur_table_base_page = Page::from_index((table_write.as_ptr() as usize) / 0x1000);
        let new_table_base_page = Page::from_index(*start_index.get_or_init(|| cur_table_len));
        // Copy the existing table's pages by simply remapping the pages to point to the existing frames.
        for page_offset in 0..cur_table_page_count {
            page_manager
                .copy_by_map(
                    &cur_table_base_page.forward_checked(page_offset).unwrap(),
                    &new_table_base_page.forward_checked(page_offset).unwrap(),
                    None,
                )
                .unwrap();
        }
        // For the remainder of the table's pages (pages that didn't exist prior), create new auto mappings.
        for page_offset in cur_table_page_count..req_table_page_count {
            let mut new_page = new_table_base_page.forward_checked(page_offset).unwrap();
            page_manager.auto_map(&new_page, PageAttributes::DATA);
            // Clear the newly allocated table page.
            unsafe { new_page.mem_clear() };
        }

        // Point to new map.
        **table_write = unsafe {
            core::slice::from_raw_parts_mut(
                new_table_base_page.as_mut_ptr(),
                liblz::align_up(req_table_len, 0x1000 / size_of::<BlockPage>()),
            )
        };
        // Always set the 0th (null) page to full by default.
        // Helps avoid some erros.
        table_write[0].set_full();
        // Mark the current table's pages as full within the table.
        table_write
            .iter_mut()
            .skip(new_table_base_page.index())
            .take(req_table_page_count)
            .for_each(|block_page| block_page.set_full());
        // Mark the old table's pages as empty within the table.
        table_write
            .iter_mut()
            .skip(cur_table_base_page.index())
            .take(cur_table_page_count)
            .for_each(|block_page| block_page.set_empty());

        Ok(())
    }
}

unsafe impl core::alloc::Allocator for SLOB<'_> {
    fn allocate(
        &self,
        layout: Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        let align_mask = usize::max(layout.align() / Self::BLOCK_SIZE, 1) - 1;
        let size_in_blocks = liblz::align_up_div(layout.size(), Self::BLOCK_SIZE);

        let mut table_write = self.table.write();
        let end_table_index;
        let mut block_index;
        let mut current_run;

        'outer: loop {
            block_index = 0;
            current_run = 0;

            for (table_index, block_page) in table_write.iter().enumerate() {
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
                                end_table_index = table_index + 1;
                                break 'outer;
                            }
                        }

                        block_index += 1;
                    }
                }
            }

            // No properly sized region was found, so grow list.
            if let Err(_) = self.grow(
                core::num::NonZeroUsize::new(size_in_blocks).unwrap(),
                &mut table_write,
            ) {
                return Err(core::alloc::AllocError);
            }
        }

        let end_block_index = block_index + 1;
        block_index -= current_run - 1;
        let start_block_index = block_index;
        let start_table_index = start_block_index / BlockPage::BLOCKS_PER;
        let page_manager = liblz::memory::global_pmgr();
        for table_index in start_table_index..end_table_index {
            let block_page = &mut table_write[table_index];
            let was_empty = block_page.is_empty();

            let block_index_floor = table_index * BlockPage::BLOCKS_PER;
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
                page_manager.auto_map(&Page::from_index(table_index), PageAttributes::DATA);
            }
        }

        unsafe {
            core::ptr::NonNull::new(core::slice::from_raw_parts_mut(
                (start_block_index * Self::BLOCK_SIZE) as *mut u8,
                layout.size(),
            ))
            .ok_or(core::alloc::AllocError)
        }
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: Layout) {
        let start_block_index = ptr.addr().get() / Self::BLOCK_SIZE;
        let end_block_index = start_block_index + align_up_div(layout.size(), Self::BLOCK_SIZE);
        let mut block_index = start_block_index;
        trace!(
            "Deallocation requested: {}..{}",
            start_block_index,
            end_block_index
        );

        let start_table_index = start_block_index / BlockPage::BLOCKS_PER;
        let end_table_index = align_up_div(end_block_index, BlockPage::BLOCKS_PER);
        let mut table_write = self.table.write();
        let page_manager = liblz::memory::global_pmgr();
        for map_index in start_table_index..end_table_index {
            let (had_bits, has_bits) = {
                let block_page = &mut table_write[map_index];

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
                page_manager
                    .unmap(
                        &Page::from_index(map_index),
                        liblz::memory::FrameOwnership::Locked,
                    )
                    .unwrap();
            }
        }
    }
}

unsafe impl core::alloc::GlobalAlloc for SLOB<'_> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match <Self as core::alloc::Allocator>::allocate(&self, layout) {
            Ok(non_null) => non_null.as_mut_ptr(),
            Err(_) => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        <Self as core::alloc::Allocator>::deallocate(
            &self,
            core::ptr::NonNull::new(ptr).unwrap(),
            layout,
        )
    }
}
