use crate::memory::{PageAttributes, PageManager};
use core::{alloc::Layout, mem::size_of};
use libkernel::{align_up_div, memory::Page};
use spin::{RwLock, RwLockWriteGuard};

/// Represents one page worth of memory blocks (i.e. 4096 bytes in blocks).
#[repr(transparent)]
#[derive(Clone)]
struct BlockPage(u64);

impl BlockPage {
    /// How many bits/block indexes in section primitive.
    const BLOCKS_PER: usize = u64::BITS as usize;

    /// Whether the block page is empty.
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.0 == u64::MIN
    }

    /// Whether the block page is full.
    #[inline(always)]
    pub const fn is_full(&self) -> bool {
        self.0 == u64::MAX
    }

    /// Unset all of the block page's blocks.
    #[inline(always)]
    pub fn set_empty(&mut self) {
        self.0 = u64::MIN;
    }

    /// Set all of the block page's blocks.
    #[inline(always)]
    pub fn set_full(&mut self) {
        self.0 = u64::MAX;
    }

    #[inline(always)]
    pub const fn value(&self) -> &u64 {
        &self.0
    }

    #[inline(always)]
    pub fn value_mut(&mut self) -> &mut u64 {
        &mut self.0
    }
}

impl core::fmt::Debug for BlockPage {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("BlockPage").field(&format_args!("0b{:b}", self.0)).finish()
    }
}

pub enum AllocError {
    OutOfMemory,
}

/// Allocator utilizing blocks of memory, in size of 64 bytes per block, to
///  easily and efficiently allocate.
pub struct SLOB<'map> {
    table: RwLock<&'map mut [BlockPage]>,
    base_alloc_page: Page,
}

impl<'map> SLOB<'map> {
    /// The size of an allocator block.
    pub const BLOCK_SIZE: usize = 0x1000 / BlockPage::BLOCKS_PER;

    pub unsafe fn new(base_alloc_page: Page) -> Self {
        let alloc_table_len = 0x1000 / core::mem::size_of::<BlockPage>();
        let current_page_manager = PageManager::from_current(&crate::memory::get_kernel_hhdm_page());
        let kernel_frame_manager = crate::memory::get_kernel_frame_manager();

        // Map all of the pages in the allocation table.
        for page_offset in 0..alloc_table_len {
            let new_page = base_alloc_page.forward_checked(page_offset).unwrap();
            current_page_manager.auto_map(&new_page, PageAttributes::RW, kernel_frame_manager);
        }

        let alloc_table = core::slice::from_raw_parts_mut(
            // Ensure when we map, we utilize the allocator's base table address.
            base_alloc_page.address().as_mut_ptr::<BlockPage>(),
            alloc_table_len,
        );
        // Fill the allocator table's page.
        alloc_table[0].set_full();

        Self { table: RwLock::new(alloc_table), base_alloc_page }
    }

    /// Calculates the bit count and mask for a given set of block page parameters.
    fn calculate_bit_fields(map_index: usize, cur_block_index: usize, end_block_index: usize) -> (usize, u64) {
        let floor_block_index = map_index * BlockPage::BLOCKS_PER;
        let ceil_block_index = floor_block_index + BlockPage::BLOCKS_PER;
        let mask_bit_offset = cur_block_index - floor_block_index;
        let mask_bit_count = usize::min(ceil_block_index, end_block_index) - cur_block_index;

        // SAFETY: The above calculations for `floor_block_index` and `ceil_block_index` ensure the shift will be <64.
        let mask_bits = unsafe { u64::MAX.unchecked_shr((u64::BITS as u64) - (mask_bit_count as u64)) }
            .checked_shl(mask_bit_offset as u32)
            .unwrap();

        (mask_bit_count, mask_bits)
    }

    fn grow(
        required_blocks: core::num::NonZeroUsize,
        table: &mut RwLockWriteGuard<&mut [BlockPage]>,
        base_alloc_page: &Page,
    ) -> Result<(), AllocError> {
        // Current length of our map, in indexes.
        let cur_table_len = table.len();
        // Required length of our map, in indexes.
        let req_table_len =
            (table.len() + libkernel::align_up_div(required_blocks.get(), BlockPage::BLOCKS_PER)).next_power_of_two();
        // Current page count of our map (i.e. how many pages the slice requires)
        let cur_table_page_count = libkernel::align_up_div(cur_table_len * size_of::<BlockPage>(), 0x1000);
        // Required page count of our map.
        let req_table_page_count = libkernel::align_up_div(req_table_len * size_of::<BlockPage>(), 0x1000);

        if (req_table_len * 0x1000) >= 0x400000000000 {
            return Err(AllocError::OutOfMemory);
        }

        // Attempt to find a run of already-mapped pages within our allocator that can contain
        // the required slice length.
        let mut current_run = 0;
        let start_index = core::cell::OnceCell::new();
        for (index, block_page) in table.iter().enumerate() {
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

        // Map the new table extents. Each table index beyond `cur_table_len` is a new page.
        {
            let frame_manager = crate::memory::get_kernel_frame_manager();
            // SAFETY: Kernel guarantees the HHDM will be a valid and mapped address.
            let page_manager = unsafe { PageManager::from_current(&crate::memory::get_kernel_hhdm_page()) };

            for page_offset in cur_table_len..req_table_len {
                let new_page = base_alloc_page.forward_checked(page_offset).unwrap();
                page_manager.auto_map(&new_page, PageAttributes::RW, frame_manager);
                // Clear the newly allocated table page.
                // SAFETY: We know no important memory is stored here to be overwritten, because we just mapped it.
                unsafe { new_page.clear_memory() };
            }
        }

        let cur_table_start_index =
            ((table.as_ptr() as usize) - base_alloc_page.address().as_usize()) / Self::BLOCK_SIZE;
        let new_table_start_index = *start_index.get_or_init(|| cur_table_len);
        // Ensure we set the new base table page to use the base allocation page as a starting index.
        let new_table_base_page = base_alloc_page.forward_checked(new_table_start_index).unwrap();

        let new_table =
        // SAFETY: We know the address is pointer-aligned, and that the address range is valid for clearing via `write_bytes`.        
        unsafe {
            let new_table_ptr = new_table_base_page.address().as_mut_ptr::<BlockPage>();
            // Ensure we clear the new table's contents before making a slice of it.
            core::ptr::write_bytes(new_table_ptr, 0, req_table_len);
            core::slice::from_raw_parts_mut(new_table_ptr, req_table_len)
        };

        // Copy old table into new table.
        //
        // REMARK: This could be done via `page_manager.copy_by_map`, I believe, but that approach introduces a certain level
        //         of indirection which can make the overall process very confusing. If this block of code proves to be a performance
        //         bottleneck, such an approach could be employed. However, given this function runs very infrequently, I find it hard
        //         to imagine this will bottleneck allocations.
        table.iter_mut().enumerate().for_each(|(index, block_page)| new_table[index] = block_page.clone());

        // Clear old table bytes.
        unsafe {
            let old_table_ptr = table.as_mut_ptr();
            core::ptr::write_bytes(old_table_ptr, 0, table.len());
        }

        // Point to new map.
        **table = new_table;
        // Mark the new table's pages as full within the table.
        table.iter_mut().skip(new_table_start_index).take(req_table_page_count).for_each(BlockPage::set_full);
        // Mark the old table's pages as empty within the table.
        table.iter_mut().skip(cur_table_start_index).take(cur_table_page_count).for_each(BlockPage::set_empty);

        Ok(())
    }
}

unsafe impl core::alloc::Allocator for SLOB<'_> {
    fn allocate(&self, layout: Layout) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        crate::interrupts::without(|| {
            let mut table = self.table.write();

            let align_mask = usize::max(layout.align() / Self::BLOCK_SIZE, 1) - 1;
            let size_in_blocks = libkernel::align_up_div(layout.size(), Self::BLOCK_SIZE);

            let end_table_index;
            let mut block_index;
            let mut current_run;

            'outer: loop {
                block_index = 0;
                current_run = 0;

                for (table_index, block_page) in table.iter().enumerate() {
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
                if Self::grow(core::num::NonZeroUsize::new(size_in_blocks).unwrap(), &mut table, &self.base_alloc_page)
                    .is_err()
                {
                    return Err(core::alloc::AllocError);
                }
            }

            let end_block_index = block_index + 1;
            block_index -= current_run - 1;
            let start_block_index = block_index;
            let start_table_index = start_block_index / BlockPage::BLOCKS_PER;
            for table_index in start_table_index..end_table_index {
                let block_page = &mut table[table_index];

                let (bit_count, bit_mask) = Self::calculate_bit_fields(table_index, block_index, end_block_index);
                debug_assert_eq!(*block_page.value() & bit_mask, 0);

                *block_page.value_mut() |= bit_mask;
                debug_assert_eq!(*block_page.value() & bit_mask, bit_mask);

                block_index += bit_count;
            }

            let allocation_ptr =
                (self.base_alloc_page.address().as_usize() + (start_block_index * Self::BLOCK_SIZE)) as *mut u8;
            core::ptr::NonNull::new(core::ptr::slice_from_raw_parts_mut(allocation_ptr, layout.size()))
                .ok_or(core::alloc::AllocError)
        })
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: Layout) {
        debug_assert!(ptr.as_ptr().is_aligned_to(Self::BLOCK_SIZE));

        crate::interrupts::without(|| {
            let mut table = self.table.write();

            let start_block_index = (ptr.addr().get() - self.base_alloc_page.address().as_usize()) / Self::BLOCK_SIZE;
            let end_block_index = start_block_index + align_up_div(layout.size(), Self::BLOCK_SIZE);
            let mut block_index = start_block_index;

            let start_table_index = start_block_index / BlockPage::BLOCKS_PER;
            let end_table_index = align_up_div(end_block_index, BlockPage::BLOCKS_PER);
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

/// SAFETY: Honestly, I've probably fucked up some of the invariants `GlobalAlloc` is supposed to provide.
unsafe impl core::alloc::GlobalAlloc for SLOB<'_> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match <Self as core::alloc::Allocator>::allocate(self, layout) {
            Ok(non_null) => non_null.as_mut_ptr(),
            Err(_) => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        <Self as core::alloc::Allocator>::deallocate(self, core::ptr::NonNull::new(ptr).unwrap(), layout);
    }
}
