use core::{alloc::Layout, mem::size_of, num::NonZeroUsize};
use libstd::{
    addr_ty::{Physical, Virtual},
    align_up_div,
    memory::{
        falloc,
        malloc::{Alloc, AllocError, MemoryAllocator},
        paging::VirtualAddressor,
        Frame, Page,
    },
    Address, SYSTEM_SLICE_SIZE,
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

impl BlockAllocator<'_> {
    /// The size of an allocator block.
    pub const BLOCK_SIZE: usize = 0x1000 / BlockPage::BLOCKS_PER;

    /// Base page the allocator uses to store the internal block page map.
    const ALLOCATOR_BASE: Page =
        Page::from_addr(unsafe { Address::new_unsafe(SYSTEM_SLICE_SIZE * 0xA) });

    // TODO possibly move the initialization code from `init()` into this `new()` function.
    #[allow(const_item_mutation)]
    pub const fn new() -> Self {
        const EMPTY: [BlockPage; 0] = [];

        Self {
            // TODO make addressor use a RwLock
            map: RwLock::new(AllocatorMap {
                addressor: VirtualAddressor::null(),
                pages: &mut EMPTY,
            }),
        }
    }

    /* INITIALIZATION */

    pub unsafe fn init(&self, memory_map: &[libstd::memory::UEFIMemoryDescriptor]) {
        {
            debug!("Initializing allocator's virtual addressor...");
            let mut map_write = self.map.write();
            map_write.addressor = VirtualAddressor::new(Page::null());

            debug!("Identity mapping all reserved global memory frames...");
            falloc::get()
                .iter()
                .enumerate()
                .filter(|(_, state)| state.eq(&falloc::FrameState::Reserved))
                .for_each(|(index, _)| {
                    map_write.addressor.identity_map(&Frame::from_index(index));
                });

            // Since we're using physical offset mapping for our page table modification
            //  strategy, the memory needs to be identity mapped at the correct offset.
            let phys_mapping_addr = falloc::virtual_map_offset();
            debug!("Mapping physical memory at offset: {:?}", phys_mapping_addr);
            map_write
                .addressor
                .modify_mapped_page(Page::from_addr(phys_mapping_addr));

            // Swap the PML4 into CR3
            info!("Writing kernel addressor's PML4 to the CR3 register.");
            map_write.addressor.swap_into();

            debug!("Allocating reserved global memory frames...");
            falloc::get()
                .iter()
                .enumerate()
                .filter(|(_, state)| state.eq(&falloc::FrameState::Reserved))
                .for_each(|(index, _)| {
                    while map_write.pages.len() <= index {
                        self.grow(
                            usize::max(index - map_write.pages.len(), 1) * BlockPage::BLOCKS_PER,
                            &mut map_write,
                        );
                    }

                    map_write.pages[index].set_full();
                });
        }

        // 2MiB
        const STACK_SIZE: usize = 2000000;

        debug!("Heap-allocating dynamic kernel stack...");
        let stack_descriptor = memory_map
            .iter()
            .find(|descriptor| descriptor.is_stack_descriptor())
            .expect("No origin stack detected.")
            .clone();
        let org_stack_size = (stack_descriptor.page_count * 0x1000) as usize;
        let org_stack_top = stack_descriptor.phys_start.as_usize() as *const u8;
        let org_top_dyn_top_rel = self
            .alloc(STACK_SIZE, NonZeroUsize::new(1))
            .unwrap()
            .into_parts()
            .0
            .add(STACK_SIZE - org_stack_size);

        debug!("Copying origin stack contents to dynamic stack...");
        // Finally, copy the old identity-mapped stack.
        core::ptr::copy_nonoverlapping(org_stack_top, org_top_dyn_top_rel, org_stack_size);

        debug!("Adjusting stack pointer to dynamic stack...");
        // Determine offset between the two stacks, to properly move RSP.
        let stack_ptr_offset = org_stack_top.offset_from(org_top_dyn_top_rel);

        use libstd::registers::stack::RSP;
        if stack_ptr_offset.is_positive() {
            RSP::sub(stack_ptr_offset.abs() as u64);
        } else {
            RSP::add(stack_ptr_offset.abs() as u64);
        }

        debug!("Unmapping origin stack & unreserving frames...");
        let stack_descriptor = (&raw const stack_descriptor).read_volatile();
        let mut map_write = self.map.write();
        let start_index = stack_descriptor.phys_start.frame_index();
        let count = stack_descriptor.page_count as usize;
        for frame_index in start_index..(start_index + count) {
            map_write
                .addressor
                .unmap(&Page::from_index(frame_index))
                .unwrap();
            // TODO also unreserve the frame which the mapping points to.
        }

        // Reserve the null page.
        map_write.pages[0].set_full();

        info!("Finished block allocator initialization.");
    }

    /* ALLOC & DEALLOC */

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
        assert!(required_blocks > 0, "calls to grow must be nonzero");

        trace!("Growing map to faciliate {} blocks.", required_blocks);
        const BLOCKS_PER_MAP_PAGE: usize = 8 /* bits per byte */ * 0x1000;
        let cur_map_len = map_write.pages.len();
        let cur_page_offset = (cur_map_len * BlockPage::BLOCKS_PER) / BLOCKS_PER_MAP_PAGE;
        let new_page_offset = (cur_page_offset
            + libstd::align_up_div(required_blocks, BLOCKS_PER_MAP_PAGE))
        .next_power_of_two();

        trace!(
            "Growing map: {}..{} pages",
            cur_page_offset,
            new_page_offset
        );

        for offset in cur_page_offset..new_page_offset {
            let map_page = &mut Self::ALLOCATOR_BASE.forward(offset).unwrap();
            let frame = match falloc::get().autolock() {
                Some(frame) => frame,
                None => return Err(AllocError::OutOfMemory),
            };
            map_write.addressor.map(map_page, &frame);
        }

        let new_map_len = new_page_offset * (0x1000 / size_of::<BlockPage>());
        unsafe {
            map_write.pages =
                core::ptr::slice_from_raw_parts_mut(Self::ALLOCATOR_BASE.as_mut_ptr(), new_map_len)
                    .as_mut()
                    .unwrap();

            core::ptr::write_bytes(
                map_write.pages[cur_map_len..].as_mut_ptr(),
                0,
                new_map_len - cur_map_len,
            );
        }

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
                        .map(
                            &Page::from_index(map_index),
                            &falloc::get().autolock().unwrap(),
                        )
                        .unwrap();
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
        let mut frames = match falloc::get().autolock_many(count) {
            Some(frames) => frames,
            None => {
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

        for page_index in start_index..(start_index + count) {
            if let Some(frame) = frames.next() {
                map_write.pages[page_index].set_full();
                map_write
                    .addressor
                    .map(&Page::from_index(page_index), &frame);
            } else {
                for page_index in start_index..page_index {
                    map_write.pages[page_index].set_empty();
                }

                unsafe { falloc::get().free_frames(frames) };
                return Err(AllocError::UndefinedFailure);
            }
        }

        unsafe {
            Ok((
                frames.start().base_addr(),
                Alloc::new((start_index * 0x1000) as *mut _, count * 0x1000),
            ))
        }
    }

    fn alloc_against(
        &self,
        frame_index: usize,
        count: usize,
        acq_state: falloc::FrameState,
    ) -> Result<Alloc<u8>, AllocError> {
        let mut map_write = self.map.write();
        let mut frames =
            match unsafe { falloc::get().acquire_frames(frame_index, count, acq_state) } {
                Ok(frames) => frames,
                Err(falloc_error) => {
                    return Err(AllocError::FallocFailure(falloc_error));
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

        for page_index in start_index..(start_index + count) {
            if let Some(frame) = frames.next() {
                map_write.pages[page_index].set_full();
                map_write
                    .addressor
                    .map(&Page::from_index(page_index), &frame);
            } else {
                for page_index in start_index..page_index {
                    map_write.pages[page_index].set_empty();
                }

                unsafe { falloc::get().free_frames(frames) };
                return Err(AllocError::UndefinedFailure);
            }
        }

        unsafe { Ok(Alloc::new((start_index * 0x1000) as *mut _, count * 0x1000)) }
    }

    fn alloc_identity(
        &self,
        frame_index: usize,
        count: usize,
        acq_state: falloc::FrameState,
    ) -> Result<Alloc<u8>, AllocError> {
        let mut map_write = self.map.write();

        if map_write.pages.len() < (frame_index + count) {
            self.grow(
                (frame_index + count) * BlockPage::BLOCKS_PER,
                &mut map_write,
            )
            .unwrap();
        }

        let mut frames =
            match unsafe { falloc::get().acquire_frames(frame_index, count, acq_state) } {
                Ok(frames) => frames,
                Err(falloc_error) => {
                    return Err(AllocError::FallocFailure(falloc_error));
                }
            };

        for page_index in frame_index..(frame_index + count) {
            if let Some(frame) = frames.next() {
                map_write.pages[page_index].set_full();
                map_write.addressor.identity_map(&frame);
            } else {
                for page_index in frame_index..page_index {
                    map_write.pages[page_index].set_empty();
                }

                unsafe { falloc::get().free_frames(frames) };
                return Err(AllocError::UndefinedFailure);
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
                let page = &Page::from_index(map_index);

                unsafe {
                    falloc::get()
                        .free_frame(map.addressor.translate_page(page).unwrap())
                        .unwrap()
                };
                map.addressor.unmap(page);
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
