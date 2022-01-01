use core::{alloc::Layout, mem::size_of, num::NonZeroUsize};
use libstd::{
    addr_ty::{Physical, Virtual},
    align_up_div,
    memory::{
        falloc::{self, FrameType},
        malloc::{Alloc, AllocError, MemoryAllocator},
        paging::{PageTableEntry, VirtualAddressor},
        Page, UEFIMemoryDescriptor,
    },
    Address,
};
use spin::{RwLock, RwLockWriteGuard};

/// Represents one page worth of memory blocks (i.e. 4096 bytes in blocks).
#[repr(transparent)]
#[derive(Clone)]
struct BlockPage(u64);

impl BlockPage {
    /// How many bits/block indexes in section primitive.
    const BLOCKS_PER: usize = size_of::<u64>() * 8;

    /// An empty block page (all blocks zeroed).
    const fn empty() -> Self {
        Self { 0: 0 }
    }

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
        formatter.debug_tuple("BlockPage").field(&self.0).finish()
    }
}

pub struct AllocatorMap<'map> {
    addressor: VirtualAddressor,
    pages: &'map mut [BlockPage],
}

/// Allocator utilizing blocks of memory, in size of 16 bytes per block, to
///  easily and efficiently allocate.
pub struct BlockAllocator<'map> {
    map: RwLock<AllocatorMap<'map>>,
}

impl<'map> BlockAllocator<'map> {
    /// The size of an allocator block.
    pub const BLOCK_SIZE: usize = 0x1000 / BlockPage::BLOCKS_PER;

    // TODO possibly move the initialization code from `init()` into this `new()` function.
    #[allow(const_item_mutation)]
    pub fn new(memory_map: &[UEFIMemoryDescriptor]) -> Self {
        const EMPTY: [BlockPage; 0] = [];

        let block_malloc = Self {
            // TODO make addressor use a RwLock
            map: RwLock::new(AllocatorMap {
                addressor: VirtualAddressor::null(),
                pages: &mut EMPTY,
            }),
        };

        {
            let mut map_write = block_malloc.map.write();

            unsafe {
                debug!("Initializing allocator's virtual addressor...");
                map_write.addressor = VirtualAddressor::new(Page::null());

                // TODO the addressors shouldn't mmap all reserved frames by default.
                //  It is, for insatnce, useless in userland addressors, where ACPI tables
                //  don't need to be mapped.
                debug!("Identity mapping all reserved global memory frames...");
                falloc::get()
                    .iter()
                    .enumerate()
                    .filter(|(_, (ty, _, _))| ty.eq(&falloc::FrameType::Reserved))
                    .for_each(|(index, _)| {
                        map_write.addressor.identity_map(&Page::from_index(index));
                    });
            }

            // Since we're using physical offset mapping for our page table modification
            //  strategy, the memory needs to be identity mapped at the correct offset.
            let phys_mapping_addr = falloc::virtual_map_offset();
            debug!("Mapping physical memory at offset: {:?}", phys_mapping_addr);
            unsafe {
                map_write
                    .addressor
                    .modify_mapped_page(Page::from_addr(phys_mapping_addr));
            }

            info!("Writing kernel addressor's PML4 to the CR3 register.");
            unsafe { map_write.addressor.swap_into() };

            debug!("Allocating reserved global memory frames...");
            falloc::get()
                .iter()
                .enumerate()
                .filter(|(_, (ty, _, _))| ty.eq(&FrameType::Reserved))
                .for_each(|(index, _)| {
                    while map_write.pages.len() <= index {
                        block_malloc
                            .grow(
                                usize::max(index - map_write.pages.len(), 1)
                                    * BlockPage::BLOCKS_PER,
                                &mut map_write,
                            )
                            .unwrap();
                    }

                    map_write.pages[index].set_full();
                });

            map_write.pages[0].set_full();

            info!("Finished block allocator initialization.");
        }

        block_malloc
    }

    /// Calculates the bit count and mask for a given set of block page parameters.
    fn calculate_bit_fields(
        map_index: usize,
        cur_block_index: usize,
        end_block_index: usize,
    ) -> (usize, u64) {
        let traversed_blocks = map_index * BlockPage::BLOCKS_PER;
        let remaining_blocks = end_block_index - traversed_blocks;
        // Each block is one bit in our map, so we calculate the offset into
        //  the current section, at which our current index (`block_index`) lies.
        let bit_offset = cur_block_index - traversed_blocks;
        let bit_count = core::cmp::min(BlockPage::BLOCKS_PER, remaining_blocks) - bit_offset;
        // Finally, we acquire the respective bitmask to flip all relevant bits in
        //  our current section.

        (
            bit_count,
            libstd::U64_BIT_MASKS[bit_count - 1] << bit_offset,
        )
    }

    pub fn grow(
        &self,
        required_blocks: usize,
        map_write: &mut RwLockWriteGuard<AllocatorMap>,
    ) -> Result<(), AllocError> {
        assert!(
            map_write.addressor.is_swapped_in(),
            "Cannot modify allocator state when addressor is not active."
        );
        assert!(required_blocks > 0, "calls to grow must be nonzero");

        info!(
            "Allocator map requires growth: {} blocks required.",
            required_blocks
        );

        // Current length of our map, in indexes.
        let cur_map_len = map_write.pages.len();
        // Required length of our map, in indexes.
        let req_map_len = (map_write.pages.len()
            + libstd::align_up_div(required_blocks, BlockPage::BLOCKS_PER))
        .next_power_of_two();
        // Current page count of our map (i.e. how many pages the slice requires)
        let cur_map_pages = libstd::align_up_div(cur_map_len * size_of::<BlockPage>(), 0x1000);
        // Required page count of our map.
        let req_map_pages = libstd::align_up_div(req_map_len * size_of::<BlockPage>(), 0x1000);

        info!(
            "Growth parameters: len {} => {}, pages {} => {}",
            cur_map_len, req_map_len, cur_map_pages, req_map_pages
        );

        // Attempt to find a run of already-mapped pages within our allocator
        // that can contain our required slice length.
        let mut current_run = 0;
        let start_index = core::lazy::OnceCell::new();
        for (index, block_page) in map_write.pages.iter().enumerate() {
            if block_page.is_empty() {
                current_run += 1;

                if current_run == req_map_pages {
                    start_index.set(index - (current_run - 1));
                    break;
                }
            } else {
                current_run = 0;
            }
        }

        let falloc = falloc::get();
        let cur_map_page = Page::from_index((map_write.pages.as_ptr() as usize) / 0x1000);
        let new_map_page = Page::from_index(*start_index.get_or_init(|| {
            // When the map is zero-sized, this allows us to skip the first page in our
            // allocations (in order to keep the 0th page as null & unmapped).
            if cur_map_len == 0 {
                cur_map_len + 1
            } else {
                cur_map_len
            }
        }));

        info!("Copy mapping current map to new pages.");
        for page_offset in 0..cur_map_pages {
            map_write.addressor.copy_by_map(
                &cur_map_page.forward(page_offset).unwrap(),
                &new_map_page.forward(page_offset).unwrap(),
            );
        }

        info!("Allocating and mapping remaining pages of map.");
        for page_offset in cur_map_pages..req_map_pages {
            let mut new_page = new_map_page.forward(page_offset).unwrap();

            map_write.addressor.automap(&new_page, true);
            // Clear the newly allocated map page.
            unsafe { new_page.mem_clear() };
        }

        // Point to new map.
        map_write.pages = unsafe {
            core::slice::from_raw_parts_mut(
                new_map_page.as_mut_ptr(),
                libstd::align_up(req_map_len, 0x1000 / size_of::<BlockPage>()),
            )
        };

        map_write
            .pages
            .iter_mut()
            .skip(new_map_page.index())
            .take(req_map_pages)
            .for_each(|block_page| block_page.set_full());
        map_write
            .pages
            .iter_mut()
            .skip(cur_map_page.index())
            .take(cur_map_pages)
            .for_each(|block_page| block_page.set_empty());

        Ok(())
    }
}

impl MemoryAllocator for BlockAllocator<'_> {
    fn alloc(&self, size: usize, align: Option<NonZeroUsize>) -> Result<Alloc<u8>, AllocError> {
        let align = align
            .unwrap_or(unsafe { NonZeroUsize::new_unchecked(1) })
            .get();
        if !align.is_power_of_two() {
            return Err(AllocError::InvalidAlignment);
        }

        let align_shift = usize::max(align / Self::BLOCK_SIZE, 1);
        let size_in_blocks = libstd::align_up_div(size, Self::BLOCK_SIZE);

        let mut map_write = self.map.write();
        let mut end_map_index = 0;
        let mut block_index = 0;
        let mut current_run = 0;
        'outer: loop {
            current_run = 0;

            for block_page in map_write.pages.iter().skip(end_map_index) {
                if block_page.is_full() {
                    current_run = 0;
                    block_index += BlockPage::BLOCKS_PER;
                } else {
                    for bit_shift in 0..64 {
                        block_index += 1;

                        if (block_page.value() & bit_shift) > 0 {
                            current_run = 0;
                        } else if current_run > 0 || (bit_shift % (align_shift as u64)) == 0 {
                            current_run += 1;

                            if current_run == size_in_blocks {
                                break 'outer;
                            }
                        }
                    }
                }

                end_map_index += 1;
            }

            if let Err(alloc_err) = self.grow(size_in_blocks, &mut map_write) {
                return Err(alloc_err);
            }
        }

        // TODO fix the indexing on these
        let end_block_index = block_index;
        let start_block_index = block_index - current_run;
        block_index = start_block_index;
        end_map_index += 1;
        let start_map_index =
            end_map_index - libstd::align_up_div(size_in_blocks, BlockPage::BLOCKS_PER);

        for map_index in start_map_index..end_map_index {
            let block_page = &mut map_write.pages[map_index];
            let was_empty = block_page.is_empty();
            let (bit_count, bit_mask) =
                Self::calculate_bit_fields(map_index, block_index, end_block_index);

            *block_page.value_mut() |= bit_mask;
            block_index += bit_count;

            if was_empty {
                unsafe {
                    map_write
                        .addressor
                        .automap(&Page::from_index(map_index), true);
                }
            }
        }

        unsafe {
            Ok(Alloc::new(
                (start_block_index * Self::BLOCK_SIZE) as *mut _,
                size_in_blocks * Self::BLOCK_SIZE,
            ))
        }
    }

    fn alloc_contiguous(&self, count: usize) -> Result<(Address<Physical>, Alloc<u8>), AllocError> {
        let mut map_write = self.map.write();
        let frame_index = match falloc::get().lock_next_many(count) {
            Ok(frame_index) => frame_index,
            Err(falloc_err) => {
                return Err(AllocError::OutOfFrames);
            }
        };

        let mut start_index = 0;
        'outer: loop {
            let mut current_run = 0;

            for (map_index, block_page) in map_write.pages.iter_mut().enumerate().skip(start_index)
            {
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

            map_write.pages[page_index].set_full();
            map_write
                .addressor
                .map(&Page::from_index(page_index), frame_index, None);
        }

        unsafe {
            Ok((
                Address::<Physical>::new(frame_index * 0x1000),
                Alloc::new((start_index * 0x1000) as *mut _, count * 0x1000),
            ))
        }
    }

    fn alloc_against(&self, frame_index: usize, count: usize) -> Result<Alloc<u8>, AllocError> {
        let mut map_write = self.map.write();
        let mut start_index = 0;
        'outer: loop {
            let mut current_run = 0;

            for (map_index, block_page) in map_write.pages.iter_mut().enumerate().skip(start_index)
            {
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

            map_write.pages[page_index].set_full();
            map_write
                .addressor
                .map(&Page::from_index(page_index), frame_index, None);
        }

        unsafe { Ok(Alloc::new((start_index * 0x1000) as *mut _, count * 0x1000)) }
    }

    fn alloc_identity(&self, frame_index: usize, count: usize) -> Result<Alloc<u8>, AllocError> {
        let mut map_write = self.map.write();

        if map_write.pages.len() < (frame_index + count) {
            self.grow(
                (frame_index + count) * BlockPage::BLOCKS_PER,
                &mut map_write,
            )
            .unwrap();
        }

        for page_index in frame_index..(frame_index + count) {
            if map_write.pages[page_index].is_empty() {
                map_write.pages[page_index].set_full();
                map_write
                    .addressor
                    .identity_map(&Page::from_index(page_index));
            } else {
                for page_index in frame_index..page_index {
                    map_write.pages[page_index].set_empty();
                    map_write
                        .addressor
                        .unmap(&Page::from_index(page_index), false);
                }

                return Err(AllocError::IdentityMappingOverlaps);
            }
        }

        unsafe { Ok(Alloc::new((frame_index * 0x1000) as *mut _, count * 0x1000)) }
    }

    fn dealloc(&self, ptr: *mut u8, layout: Layout) {
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
        let mut map = self.map.write();
        for map_index in start_map_index..end_map_index {
            let (had_bits, has_bits) = {
                let block_page = &mut map.pages[map_index];

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
                // TODO we actually *don't know* if this page locked a frame or not...
                map.addressor.unmap(&Page::from_index(map_index), true);
            }
        }
    }

    fn get_page_attribs(&self, page: &Page) -> Option<libstd::memory::paging::PageAttributes> {
        unsafe { self.map.read().addressor.get_page_attribs(page) }
    }

    unsafe fn set_page_attribs(
        &self,
        page: &Page,
        attributes: libstd::memory::paging::PageAttributes,
        modify_mode: libstd::memory::paging::AttributeModify,
    ) {
        self.map
            .write()
            .addressor
            .set_page_attribs(page, attributes, modify_mode)
    }

    fn get_page_state(&self, page_index: usize) -> Option<bool> {
        self.map
            .read()
            .pages
            .get(page_index)
            .map(|block_page| !block_page.is_empty())
    }

    unsafe fn physical_memory(&self, addr: Address<Physical>) -> Address<Virtual> {
        self.map.read().addressor.mapped_offset() + addr.as_usize()
    }
}
