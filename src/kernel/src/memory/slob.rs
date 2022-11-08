use core::{
    alloc::{AllocError, Layout},
    mem::size_of,
    num::NonZeroUsize,
    ptr::NonNull,
};
use libcommon::{align_up_div, Address, Page};
use spin::Mutex;

/// Represents one page worth of memory blocks (i.e. 4096 bytes in blocks).
#[repr(transparent)]
#[derive(Clone)]
struct Block(u64);

impl Block {
    /// How many bits/block indexes in section primitive.
    const BLOCKS_PER: usize = u64::BITS as usize;

    pub const FULL: Self = Self(u64::MAX);
    pub const EMPTY: Self = Self(u64::MAX);

    /// Whether the block page is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == u64::MIN
    }

    /// Whether the block page is full.
    #[inline]
    pub const fn is_full(&self) -> bool {
        self.0 == u64::MAX
    }

    /// Unset all of the block page's blocks.
    #[inline]
    pub fn set_empty(&mut self) {
        self.0 = u64::MIN;
    }

    /// Set all of the block page's blocks.
    #[inline]
    pub fn set_full(&mut self) {
        self.0 = u64::MAX;
    }

    #[inline]
    pub const fn value(&self) -> &u64 {
        &self.0
    }

    #[inline]
    pub fn value_mut(&mut self) -> &mut u64 {
        &mut self.0
    }
}

impl core::fmt::Debug for Block {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Block").field(&format_args!("0b{:b}", self.0)).finish()
    }
}

/// Allocator utilizing blocks of memory, in size of 64 bytes per block, to
/// easily and efficiently allocate.
pub struct Slob<'a> {
    table: Mutex<&'a mut [Block]>,
    base_address: Address<Page>,
    map_page_fn: fn(Address<Page>) -> Result<(), ()>,
}

impl<'a> Slob<'a> {
    /// The size of an allocated block.
    pub const BLOCK_SIZE: usize = 0x1000 / Block::BLOCKS_PER;

    pub unsafe fn new(base_address: Address<Page>, map_page_fn: fn(Address<Page>) -> Result<(), ()>) -> Option<Self> {
        const INITIAL_TABLE_LEN: usize = 0x1000 / size_of::<Block>();

        // Map all of the pages in the allocation table.
        for page_offset in 0..INITIAL_TABLE_LEN {
            map_page_fn(base_address.forward_checked(page_offset).ok()?).ok()?;
        }

        let alloc_table = core::slice::from_raw_parts_mut(
            // Ensure when we map, we utilize the allocator's base table address.
            base_address.address().as_mut_ptr::<Block>(),
            INITIAL_TABLE_LEN,
        );
        // Fill the allocator table's page.
        alloc_table[0].set_full();

        Some(Self { table: Mutex::new(alloc_table), base_address, map_page_fn })
    }

    /// Calculates the bit count and mask for a given set of block page parameters.
    fn calculate_bit_fields(map_index: usize, cur_block_index: usize, end_block_index: usize) -> (usize, u64) {
        let floor_block_index = map_index * Block::BLOCKS_PER;
        let ceil_block_index = floor_block_index + Block::BLOCKS_PER;
        let mask_bit_offset = cur_block_index - floor_block_index;
        let mask_bit_count = usize::min(ceil_block_index, end_block_index) - cur_block_index;

        // ### Safety: The above calculations for `floor_block_index` and `ceil_block_index` ensure the shift will be <64.
        let mask_bits = unsafe { u64::MAX.unchecked_shr((u64::BITS as u64) - (mask_bit_count as u64)) }
            .checked_shl(mask_bit_offset as u32)
            .unwrap();

        (mask_bit_count, mask_bits)
    }

    fn grow(
        required_blocks: NonZeroUsize,
        table: &mut [Block],
        base_address: Address<Page>,
        map_page_fn: impl FnMut(Address<Page>) -> Result<(), ()>,
    ) -> Result<(), AllocError> {
        // Current length of our map, in indexes.
        let current_table_len = table.len();
        // Required length of our map, in indexes.
        let required_table_len =
            (table.len() + libcommon::align_up_div(required_blocks.get(), Block::BLOCKS_PER)).next_power_of_two();
        if (required_table_len * 0x1000) >= 0x400000000000 {
            return Err(AllocError);
        }

        // Current page count of our map (i.e. how many pages the slice requires)
        let cur_table_page_count = libcommon::align_up_div(current_table_len * size_of::<Block>(), 0x1000);
        // Required page count of our map.
        let required_run = libcommon::align_up_div(required_table_len * size_of::<Block>(), 0x1000);

        // Attempt to find a run of already-mapped pages within our allocator that can contain
        // the required slice length.
        let mut current_run = 0;
        let mut block_iter = table.iter().enumerate();
        let start_index = {
            loop {
                let (index, block) = block_iter.next()?;

                if block.is_empty() {
                    current_run += 1;

                    if current_run == required_run {
                        break Some(index - (current_run - 1));
                    }
                } else {
                    current_run = 0;
                }
            }
        };

        // Map the new table extents. Each table index beyond `cur_table_len` is a new page.
        {
            for page_offset in current_table_len..required_table_len {
                let new_page = base_address.forward_checked(page_offset).ok_or(AllocError)?;
                map_page_fn(new_page).map_err(AllocError)?;
                // Clear the newly allocated table page.
                // ### Safety: We know no important memory is stored here to be overwritten, because we just mapped it.
                unsafe { new_page.zero_memory() };
            }
        }

        let cur_table_start_index = (table.as_ptr().addr() - base_address.address().as_usize()) / Self::BLOCK_SIZE;
        let new_table_start_index = *start_index.get_or_init(|| current_table_len);
        // Ensure we set the new base table page to use the base allocation page as a starting index.
        let new_table_base_page = base_address.forward_checked(new_table_start_index).unwrap();

        let new_table =
        // ### Safety: We know the address is pointer-aligned, and that the address range is valid for clearing via `write_bytes`.        
        unsafe {
            let new_table_ptr = new_table_base_page.address().as_mut_ptr::<Block>();
            // Ensure we clear the new table's contents before making a slice of it.
            core::ptr::write_bytes(new_table_ptr, 0, required_table_len);
            core::slice::from_raw_parts_mut(new_table_ptr, required_table_len)
        };

        // Copy old table into new table.
        (&mut new_table[..table.len()]).copy_from_slice(table);

        // Clear old table bytes.
        // ### Safety: Old table memory is no longer used.
        unsafe {
            core::ptr::write_bytes(table.as_mut_ptr(), 0, table.len());
        }

        // Point to new map.
        **table = new_table;
        // Mark the new table's pages as full within the table.
        (&mut table[new_table_start_index..(new_table_start_index + required_run)]).fill(Block::FULL);
        // Mark the old table's pages as empty within the table.
        (&mut table[cur_table_start_index..(cur_table_start_index + cur_table_page_count)]).fill(Block::EMPTY);

        Ok(())
    }
}

unsafe impl core::alloc::Allocator for Slob<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        crate::interrupts::without(|| {
            let mut table = self.table.lock();

            let align_mask = usize::max(layout.align() / Self::BLOCK_SIZE, 1) - 1;
            let size_in_blocks = libcommon::align_up_div(layout.size(), Self::BLOCK_SIZE);

            let end_table_index;
            let mut block_index;
            let mut current_run;

            'outer: loop {
                block_index = 0;
                current_run = 0;

                for (table_index, block_page) in table.iter().enumerate() {
                    if block_page.is_full() {
                        current_run = 0;
                        block_index += Block::BLOCKS_PER;
                    } else {
                        for bit_shift in 0..Block::BLOCKS_PER {
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
                Self::grow(
                    // ### Safety: Value is known to be non-zero.
                    unsafe { NonZeroUsize::new_unchecked(size_in_blocks) },
                    &mut *table,
                    self.base_address,
                    self.map_page_fn,
                )?;
            }

            let end_block_index = block_index + 1;
            block_index -= current_run - 1;
            let start_block_index = block_index;
            let start_table_index = start_block_index / Block::BLOCKS_PER;
            for table_index in start_table_index..end_table_index {
                let block_page = &mut table[table_index];

                let (bit_count, bit_mask) = Self::calculate_bit_fields(table_index, block_index, end_block_index);
                debug_assert_eq!(*block_page.value() & bit_mask, 0);

                *block_page.value_mut() |= bit_mask;
                debug_assert_eq!(*block_page.value() & bit_mask, bit_mask);

                block_index += bit_count;
            }

            let allocation_ptr =
                (self.base_address.address().as_usize() + (start_block_index * Self::BLOCK_SIZE)) as *mut u8;
            NonNull::new(core::ptr::slice_from_raw_parts_mut(allocation_ptr, layout.size())).ok_or(AllocError)
        })
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        debug_assert!(ptr.as_ptr().is_aligned_to(Self::BLOCK_SIZE));

        crate::interrupts::without(|| {
            let mut table = self.table.write();

            let start_block_index = (ptr.addr().get() - self.base_address.address().as_usize()) / Self::BLOCK_SIZE;
            let end_block_index = start_block_index + align_up_div(layout.size(), Self::BLOCK_SIZE);
            let mut block_index = start_block_index;

            let start_table_index = start_block_index / Block::BLOCKS_PER;
            let end_table_index = align_up_div(end_block_index, Block::BLOCKS_PER);
            for table_index in start_table_index..end_table_index {
                let block_page = &mut table[table_index];

                let (bit_count, bit_mask) = Self::calculate_bit_fields(table_index, block_index, end_block_index);
                debug_assert_eq!(*block_page.value() & bit_mask, bit_mask);

                *block_page.value_mut() ^= bit_mask;
                debug_assert_eq!(*block_page.value() & bit_mask, 0);

                block_index += bit_count;
            }
        });
    }
}
