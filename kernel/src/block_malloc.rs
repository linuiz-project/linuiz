use core::{alloc::Layout, mem::size_of};
use libkernel::{
    addr_ty::{Physical, Virtual},
    align_up_div,
    memory::{falloc, paging::VirtualAddressor, Frame, FrameIterator, Page},
    Address, SYSTEM_SLICE_SIZE,
};
use spin::RwLock;

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

/// Allows tracking the state of the current block page's section
///  in a loop, so a block page's underlying global memory can be
///  allocated or deallocated accordingly.
#[derive(Clone, Copy)]
struct SectionState {
    had_bits: bool,
    has_bits: bool,
}

impl SectionState {
    /// An empty section state.
    const fn empty() -> Self {
        Self {
            had_bits: false,
            has_bits: false,
        }
    }

    /// Whether the section state indicates an empty section.
    const fn is_empty(&self) -> bool {
        !self.had_bits && !self.has_bits
    }

    /// Whether the section states indicates a section that should be allocated.
    const fn is_alloc(&self) -> bool {
        !self.had_bits && self.has_bits
    }

    /// Whether the section states indicates a section that should be deallocated.
    const fn is_dealloc(&self) -> bool {
        self.had_bits && !self.has_bits
    }

    /// Whether the given block page section states indicate an allocation.
    fn should_alloc(page_state: &[SectionState]) -> bool {
        page_state.iter().any(|state| state.is_alloc())
            && page_state
                .iter()
                .all(|state| state.is_alloc() || state.is_empty())
    }

    /// Whether the given block page section states indicate an deallocation.
    fn should_dealloc(page_state: &[SectionState]) -> bool {
        page_state.iter().any(|state| state.is_dealloc())
            && page_state
                .iter()
                .all(|state| state.is_dealloc() || state.is_empty())
    }
}

impl core::fmt::Debug for SectionState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("SectionState")
            .field(&self.had_bits)
            .field(&self.has_bits)
            .finish()
    }
}

/// Allocator utilizing blocks of memory, in size of 16 bytes per block, to
///  easily and efficiently allocate.
pub struct BlockAllocator<'map> {
    // todo remove addressor from this struct
    addressor: RwLock<VirtualAddressor>,
    map: RwLock<&'map mut [BlockPage]>,
}

impl BlockAllocator<'_> {
    /// The size of an allocator block.
    pub const BLOCK_SIZE: usize = 0x1000 / BlockPage::BLOCKS_PER;

    /// Base page the allocator uses to store the internal block page map.
    const ALLOCATOR_BASE: Page =
        Page::from_addr(unsafe { Address::new_unsafe(SYSTEM_SLICE_SIZE * 0xA) });

    #[allow(const_item_mutation)]
    pub const fn new() -> Self {
        const EMPTY: [BlockPage; 0] = [];

        Self {
            // TODO make addressor use a RwLock
            addressor: RwLock::new(VirtualAddressor::null()),
            map: RwLock::new(&mut EMPTY),
        }
    }

    pub fn get_addressor(&self) -> spin::RwLockReadGuard<VirtualAddressor> {
        self.addressor.read()
    }

    pub unsafe fn get_addressor_mut(&self) -> spin::RwLockWriteGuard<VirtualAddressor> {
        self.addressor.write()
    }

    /* INITIALIZATION */

    pub unsafe fn init(&self, mut stack_frames: libkernel::memory::FrameIterator) {
        {
            debug!("Initializing allocator's virtual addressor.");
            let mut addressor_mut = self.get_addressor_mut();
            *addressor_mut = VirtualAddressor::new(Page::null());

            debug!("Identity mapping all reserved global memory frames.");

            falloc::get()
                .iter()
                .enumerate()
                .filter(|(_, frame_state)| *frame_state == falloc::FrameState::NonUsable)
                .for_each(|(frame_index, _)| {
                    addressor_mut.identity_map(&Frame::from_index(frame_index))
                });

            // Since we're using physical offset mapping for our page table modification
            //  strategy, the memory needs to be identity mapped at the correct offset.
            let phys_mapping_addr = falloc::virtual_map_offset();
            debug!("Mapping physical memory at offset: {:?}", phys_mapping_addr);
            addressor_mut.modify_mapped_page(Page::from_addr(phys_mapping_addr));

            // Swap the PML4 into CR3
            debug!("Writing kernel addressor's PML4 to the CR3 register.");
            addressor_mut.swap_into();
        }

        debug!("Allocating reserved global memory frames.");
        falloc::get()
            .iter()
            .enumerate()
            .filter(|(_, frame_state)| *frame_state == falloc::FrameState::NonUsable)
            .for_each(|(frame_index, _)| self.identity_map(&Frame::from_index(frame_index), false));

        let stack_frame_count = stack_frames.len();
        let new_stack_size = stack_frame_count * 0x1000;
        debug!("Allocating new stack: {} bytes", new_stack_size);
        let old_stack_base = stack_frames.start().base_addr().as_usize() as *const u8;
        let new_stack_base = self.alloc::<u8>(Layout::from_size_align_unchecked(new_stack_size, 1));

        debug!(
            "Copying data from bootloader-allocated stack:\n\t{:?}:{} -> {:?}",
            old_stack_base, stack_frame_count, new_stack_base
        );
        core::ptr::copy_nonoverlapping(old_stack_base, new_stack_base, new_stack_size);
        // TODO also zero all antecedent stack frames ?

        // Determine offset between the two stacks, to properly move RSP.
        let base_offset = old_stack_base.offset_from(new_stack_base);
        debug!("Modifying `rsp` by base offset: 0x{:x}.", base_offset);
        use libkernel::registers::stack::RSP;
        if base_offset.is_positive() {
            RSP::sub(base_offset.abs() as u64);
        } else {
            RSP::add(base_offset.abs() as u64);
        }

        debug!("Unmapping bootloader-provided stack frames.");
        let mut addressor_mut = self.get_addressor_mut();
        // We must copy the old stack frames iterator to our new stack; it will become unmapped
        //  as we unmap the old stack (which it exists on).
        let mut fresh_stack_frames = ((&mut stack_frames) as *mut FrameIterator).read_volatile();
        fresh_stack_frames.reset();

        for frame in fresh_stack_frames {
            addressor_mut.unmap(&Page::from_index(frame.index()));
        }

        debug!("Finished block allocator initialization.");
    }

    /* ALLOC & DEALLOC */

    // TODO consider returning a slice from this function rather than a raw pointer
    //      reasoning: possibly a more idiomatic way to return a sized chunk of memory
    pub fn alloc<T>(&self, layout: Layout) -> *mut T {
        const MINIMUM_ALIGNMENT: usize = 16;

        let size_in_blocks = (layout.size() + (Self::BLOCK_SIZE - 1)) / Self::BLOCK_SIZE;
        let alignment = if (layout.align() & (MINIMUM_ALIGNMENT - 1)) == 0 {
            layout.align()
        } else {
            trace!(
                "Unsupported allocator alignment: {}, defaulting to {}",
                layout.align(),
                Self::BLOCK_SIZE
            );

            MINIMUM_ALIGNMENT
        };

        trace!(
            "Allocation requested: {}{{by {}}} bytes ({} blocks)",
            layout.size(),
            alignment,
            size_in_blocks
        );

        let (mut block_index, mut current_run);
        while {
            block_index = 0;
            current_run = 0;

            'outer: for block_page in self.map.read().iter() {
                if block_page.is_full() {
                    current_run = 0;
                    block_index += BlockPage::BLOCKS_PER;
                } else {
                    use bit_field::BitField;

                    for bit in (0..64).map(|shift| block_page.value().get_bit(shift)) {
                        if bit {
                            current_run = 0;
                        } else if current_run > 0 || (block_index % alignment) == 0 {
                            current_run += 1;
                        }

                        block_index += 1;

                        if current_run == size_in_blocks {
                            break 'outer;
                        }
                    }
                }
            }

            current_run < size_in_blocks
        } {
            self.grow(size_in_blocks);
        }

        let start_block_index = block_index - current_run;
        let end_block_index = block_index;
        block_index = start_block_index;
        trace!(
            "Allocation fulfilling: {}..{}",
            start_block_index,
            end_block_index
        );

        let start_map_index = start_block_index / BlockPage::BLOCKS_PER;

        for (map_index, block_page) in self
            .map
            .write()
            .iter_mut()
            .enumerate()
            .skip(start_map_index)
            .take(align_up_div(end_block_index, BlockPage::BLOCKS_PER) - start_map_index)
        {
            let had_bits = !block_page.is_empty();

            let (bit_count, bit_mask) =
                Self::calculate_bit_fields(map_index, end_block_index, block_index);
            assert_eq!(
                *block_page.value() & bit_mask,
                0,
                "attempting to allocate blocks that are already allocated"
            );

            *block_page.value_mut() |= bit_mask;
            block_index += bit_count;

            let has_bits = !block_page.is_empty();

            if !had_bits && has_bits {
                let page = &mut Page::from_index(map_index);

                unsafe {
                    self.get_addressor_mut()
                        .map(page, &falloc::get().lock_next().unwrap());
                    page.clear();
                }
            }
        }

        (start_block_index * Self::BLOCK_SIZE) as *mut T
    }

    pub fn dealloc<T>(&self, ptr: *mut T, size: usize) {
        let start_block_index = (ptr as usize) / Self::BLOCK_SIZE;
        let end_block_index = start_block_index + align_up_div(size, Self::BLOCK_SIZE);
        let mut block_index = start_block_index;
        trace!(
            "Deallocation requested: {}..{}",
            start_block_index,
            end_block_index
        );

        let start_map_index = start_block_index / BlockPage::BLOCKS_PER;
        let end_map_index = align_up_div(end_block_index, BlockPage::BLOCKS_PER) - start_map_index;
        for (map_index, block_page) in self
            .map
            .write()
            .iter_mut()
            .enumerate()
            .skip(start_map_index)
            .take(end_map_index)
        {
            let had_bits = !block_page.is_empty();

            let (bit_count, bit_mask) =
                Self::calculate_bit_fields(map_index, end_block_index, block_index);

            assert_eq!(
                *block_page.value() & bit_mask,
                bit_mask,
                "attempting to deallocate blocks that are already deallocated"
            );

            *block_page.value_mut() ^= bit_mask;
            block_index += bit_count;

            let has_bits = !block_page.is_empty();

            if had_bits && !has_bits {
                let mut addressor_mut = unsafe { self.get_addressor_mut() };
                let page = &Page::from_index(map_index);
                // todo FIX THIS (uncomment & build for error)
                unsafe {
                    falloc::get()
                        .free_frame(addressor_mut.translate_page(page).unwrap())
                        .unwrap()
                };
                addressor_mut.unmap(page);
            }
        }
    }

    /// Calculates the bit count and mask for a given set of block page parameters.
    fn calculate_bit_fields(
        map_index: usize,
        end_block_index: usize,
        block_index: usize,
    ) -> (usize, u64) {
        let traversed_blocks = map_index * BlockPage::BLOCKS_PER;
        let remaining_blocks = end_block_index - traversed_blocks;
        // Each block is one bit in our map, so we calculate the offset into
        //  the current section, at which our current index (`block_index`) lies.
        let bit_offset = block_index - traversed_blocks;
        let bit_count = core::cmp::min(BlockPage::BLOCKS_PER, remaining_blocks) - bit_offset;
        // Finally, we acquire the respective bitmask to flip all relevant bits in
        //  our current section.
        (
            bit_count,
            libkernel::U64_BIT_MASKS[bit_count - 1] << bit_offset,
        )
    }

    /// Allocates a region of memory pointing to the frame region indicated by
    ///  given the iterator.
    ///
    /// This function assumed the frames are already locked or otherwise valid.
    pub fn alloc_to<T>(&self, frames: &FrameIterator) -> *mut T {
        let size_in_frames = frames.len();
        trace!("Allocation requested to: {} frames", size_in_frames);
        let (mut map_index, mut current_run);

        while {
            map_index = 0;
            current_run = 0;

            for block_page in self.map.read().iter() {
                if block_page.is_empty() {
                    current_run += 1;
                } else {
                    current_run = 0;
                }

                map_index += 1;

                if current_run == size_in_frames {
                    break;
                }
            }

            current_run < size_in_frames
        } {
            self.grow(size_in_frames * BlockPage::BLOCKS_PER);
        }

        let start_index = map_index - current_run;
        trace!(
            "Allocation fulfilling: pages {}..{}",
            start_index,
            start_index + size_in_frames
        );

        {
            let mut addressor_mut = unsafe { self.get_addressor_mut() };
            for (map_index, block_page) in self
                .map
                .write()
                .iter_mut()
                .enumerate()
                .skip(start_index)
                .take(size_in_frames)
            {
                block_page.set_full();
                addressor_mut.map(&Page::from_index(map_index), &frames.start());
            }
        }

        (start_index * 0x1000) as *mut T
    }

    pub fn identity_map(&self, frame: &Frame, virtual_map: bool) {
        trace!("Identity mapping requested: {:?}", frame);

        let map_len = self.map.read().len();
        if map_len <= frame.index() {
            self.grow(((frame.index() - map_len) + 1) * BlockPage::BLOCKS_PER);
        }

        let block_page = &mut self.map.write()[frame.index()];
        block_page.set_empty();
        assert!(
            block_page.is_empty(),
            "attempting to identity map page with previously allocated blocks: {:?} (map? {})\n {:?}",
            frame,
            virtual_map,
            block_page
        );
        block_page.set_full();

        if virtual_map {
            unsafe { self.get_addressor_mut() }.identity_map(frame);
        }
    }

    pub fn grow(&self, required_blocks: usize) {
        assert!(required_blocks > 0, "calls to grow must be nonzero");

        trace!("Growing map to faciliate {} blocks.", required_blocks);
        const BLOCKS_PER_MAP_PAGE: usize = 8 /* bits per byte */ * 0x1000;
        let map_read = self.map.upgradeable_read();
        let cur_map_len = map_read.len();
        let cur_page_offset = (cur_map_len * BlockPage::BLOCKS_PER) / BLOCKS_PER_MAP_PAGE;
        let new_page_offset = (cur_page_offset
            + libkernel::align_up_div(required_blocks, BLOCKS_PER_MAP_PAGE))
        .next_power_of_two();

        trace!(
            "Growing map: {}..{} pages",
            cur_page_offset,
            new_page_offset
        );

        {
            let mut addressor_mut = unsafe { self.get_addressor_mut() };
            for offset in cur_page_offset..new_page_offset {
                let map_page = &mut Self::ALLOCATOR_BASE.offset(offset);
                addressor_mut.map(map_page, &falloc::get().lock_next().expect("out of memory"));
            }
        }

        let new_map_len = new_page_offset * (0x1000 / size_of::<BlockPage>());
        let mut map_write = map_read.upgrade();
        *map_write = unsafe {
            &mut *core::ptr::slice_from_raw_parts_mut(
                Self::ALLOCATOR_BASE.as_mut_ptr(),
                new_map_len,
            )
        };
        map_write[cur_map_len..].fill(BlockPage::empty());

        trace!(
            "Grew map: {} pages, {} block pages, {} blocks.",
            new_page_offset,
            new_map_len,
            new_map_len * BLOCKS_PER_MAP_PAGE
        );
    }

    pub unsafe fn physical_memory(&self, addr: Address<Physical>) -> Address<Virtual> {
        self.get_addressor().mapped_page().base_addr() + addr.as_usize()
    }
}

impl libkernel::memory::malloc::MemoryAllocator for BlockAllocator<'_> {
    fn minimum_alignment(&self) -> usize {
        Self::BLOCK_SIZE
    }

    unsafe fn physical_memory(&self, addr: Address<Physical>) -> Address<Virtual> {
        self.physical_memory(addr)
    }

    fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc(layout)
    }

    fn alloc_to(&self, frames: &FrameIterator) -> *mut u8 {
        self.alloc_to(frames)
    }

    fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.dealloc(ptr, layout.size());
    }

    unsafe fn modify_page_attributes(
        &self,
        page: &Page,
        attributes: libkernel::memory::paging::PageAttributes,
        mode: libkernel::memory::paging::PageAttributeModifyMode,
    ) {
        self.get_addressor_mut()
            .modify_page_attributes(page, attributes, mode);
    }
}
