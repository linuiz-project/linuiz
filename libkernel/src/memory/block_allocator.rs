use crate::{
    align_up_div,
    memory::{paging::VirtualAddressor, Frame, FrameIterator, Page},
    SYSTEM_SLICE_SIZE,
};
use core::{
    mem::size_of,
    sync::atomic::{AtomicU64, Ordering},
};
use spin::{Mutex, RwLock};

/// Represents one page worth of memory blocks (i.e. 4096 bytes in blocks).
#[repr(C)]
struct BlockPage {
    sections: [AtomicU64; Self::SECTION_COUNT],
}

impl BlockPage {
    /// Number of sections (primitive used to track blocks with its bits).
    const SECTION_COUNT: usize = 4;
    const SECTION_LEN: usize = size_of::<u64>() * 8;
    /// Number of blocks each block page contains.
    const BLOCK_COUNT: usize = Self::SECTION_COUNT * Self::SECTION_LEN;

    /// An empty block page (all blocks zeroed).
    const fn empty() -> Self {
        const ATOMIC_U64_MIN: AtomicU64 = AtomicU64::new(u64::MIN);

        Self {
            sections: [ATOMIC_U64_MIN; Self::SECTION_COUNT],
        }
    }

    /// Whether the block page is empty.
    pub fn is_empty(&self) -> bool {
        self.iter()
            .all(|section| section.load(Ordering::Acquire) == u64::MIN)
    }

    /// Whether the block page is full.
    pub fn is_full(&self) -> bool {
        self.iter()
            .all(|section| section.load(Ordering::Acquire) == u64::MAX)
    }

    /// Unset all of the block page's blocks.
    pub fn set_empty(&mut self) {
        self.iter_mut()
            .for_each(|section| section.store(u64::MIN, Ordering::Release));
    }

    /// Set all of the block page's blocks.
    pub fn set_full(&mut self) {
        self.iter_mut()
            .for_each(|section| section.store(u64::MAX, Ordering::Release));
    }

    /// Underlying section iterator.
    fn iter(&self) -> core::slice::Iter<AtomicU64> {
        self.sections.iter()
    }

    /// Underlying mutable section iterator.
    fn iter_mut(&mut self) -> core::slice::IterMut<AtomicU64> {
        self.sections.iter_mut()
    }
}

impl core::ops::Index<usize> for BlockPage {
    type Output = AtomicU64;

    fn index(&self, index: usize) -> &Self::Output {
        &self.sections[index]
    }
}

impl core::ops::IndexMut<usize> for BlockPage {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.sections[index]
    }
}

impl Clone for BlockPage {
    fn clone(&self) -> Self {
        let clone = Self::empty();

        for index in 0..BlockPage::SECTION_COUNT {
            clone[index].store(self[index].load(Ordering::Acquire), Ordering::Release);
        }

        clone
    }
}

impl core::fmt::Debug for BlockPage {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_tuple = formatter.debug_tuple("BlockPage");

        self.iter().for_each(|section| {
            debug_tuple.field(&section.load(Ordering::Acquire));
        });

        debug_tuple.finish()
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
    addressor: Mutex<core::lazy::OnceCell<VirtualAddressor>>,
    map: RwLock<&'map mut [BlockPage]>,
}

impl BlockAllocator<'_> {
    /// The size of an allocator block.
    pub const BLOCK_SIZE: usize = 16;

    /// Base page the allocator uses to store the internal block page map.
    const ALLOCATOR_BASE: Page = Page::from_addr(x86_64::VirtAddr::new_truncate(
        (SYSTEM_SLICE_SIZE as u64) * 0xA,
    ));

    /// Provides a simple mechanism in which the mask of a u64 can be acquired by bit count.
    const MASK_MAP: [u64; 64] = [
        0x1,
        0x3,
        0x7,
        0xF,
        0x1F,
        0x3F,
        0x7F,
        0xFF,
        0x1FF,
        0x3FF,
        0x7FF,
        0xFFF,
        0x1FFF,
        0x3FFF,
        0x7FFF,
        0xFFFF,
        0x1FFFF,
        0x3FFFF,
        0x7FFFF,
        0xFFFFF,
        0x1FFFFF,
        0x3FFFFF,
        0x7FFFFF,
        0xFFFFFF,
        0x1FFFFFF,
        0x3FFFFFF,
        0x7FFFFFF,
        0xFFFFFFF,
        0x1FFFFFFF,
        0x3FFFFFFF,
        0x7FFFFFFF,
        0xFFFFFFFF,
        0x1FFFFFFFF,
        0x3FFFFFFFF,
        0x7FFFFFFFF,
        0xFFFFFFFFF,
        0x1FFFFFFFFF,
        0x3FFFFFFFFF,
        0x7FFFFFFFFF,
        0xFFFFFFFFFF,
        0x1FFFFFFFFFF,
        0x3FFFFFFFFFF,
        0x7FFFFFFFFFF,
        0xFFFFFFFFFFF,
        0x1FFFFFFFFFFF,
        0x3FFFFFFFFFFF,
        0x7FFFFFFFFFFF,
        0xFFFFFFFFFFFF,
        0x1FFFFFFFFFFFF,
        0x3FFFFFFFFFFFF,
        0x7FFFFFFFFFFFF,
        0xFFFFFFFFFFFFF,
        0x1FFFFFFFFFFFFF,
        0x3FFFFFFFFFFFFF,
        0x7FFFFFFFFFFFFF,
        0xFFFFFFFFFFFFFF,
        0x1FFFFFFFFFFFFFF,
        0x3FFFFFFFFFFFFFF,
        0x7FFFFFFFFFFFFFF,
        0xFFFFFFFFFFFFFFF,
        0x1FFFFFFFFFFFFFFF,
        0x3FFFFFFFFFFFFFFF,
        0x7FFFFFFFFFFFFFFF,
        0xFFFFFFFFFFFFFFFF,
    ];

    #[allow(const_item_mutation)]
    pub const fn new() -> Self {
        const EMPTY: [BlockPage; 0] = [];

        Self {
            // TODO make addressor use a RwLock
            addressor: Mutex::new(core::lazy::OnceCell::new()),
            map: RwLock::new(&mut EMPTY),
        }
    }

    fn with_addressor<F, R>(&self, closure: F) -> R
    where
        F: FnOnce(&mut VirtualAddressor) -> R,
    {
        if let Some(addressor) = self.addressor.lock().get_mut() {
            closure(addressor)
        } else {
            panic!("addressor has not been set")
        }
    }

    /* INITIALIZATION */

    pub fn init(&self, stack_frames: impl crate::memory::FrameIterator) {
        use crate::memory::{global_memory, FrameState};

        unsafe {
            assert!(
                self.addressor
                    .lock()
                    .set(VirtualAddressor::new(Page::null()))
                    .is_ok(),
                "addressor has already been set for allocator"
            );
        }

        debug!("Identity mapping all reserved global memory frames.");
        self.with_addressor(|addressor| {
            global_memory().iter_callback(|index, frame_type| match frame_type {
                FrameState::Reserved | FrameState::Stack => {
                    addressor.identity_map(&Frame::from_index(index));
                }
                _ => {}
            });

            // Since we're using physical offset mapping for our page table modification
            //  strategy, the memory needs to be identity mapped at the correct offset.
            let phys_mapping_addr = crate::memory::global_top_offset();
            debug!("Mapping physical memory at offset: {:?}", phys_mapping_addr);
            addressor.modify_mapped_page(Page::from_addr(phys_mapping_addr));

            unsafe {
                // Swap the PML4 into CR3
                debug!("Writing kernel addressor's PML4 to the CR3 register.");
                addressor.swap_into();
            }
        });

        global_memory().iter_callback(|index, frame_type| match frame_type {
            FrameState::Reserved => {
                self.identity_map(&Frame::from_index(index), false);
            }
            _ => {}
        });

        debug!("Allocating space for moving stack.");
        unsafe {
            let cur_stack_base = (stack_frames.clone()..start.index() * 0x1000) as u64;
            let stack_ptr = self.alloc_to(stack_frames) as u64;

            if cur_stack_base > stack_ptr {
                crate::registers::stack::RSP::sub(cur_stack_base - stack_ptr);
            } else {
                crate::registers::stack::RSP::add(stack_ptr - cur_stack_base);
            }
        }

        self.with_addressor(|addressor| {
            for frame in stack_frames {
                addressor.unmap(&Page::from_index(frame.index()));
            }
        });
    }

    /* ALLOC & DEALLOC */

    fn raw_alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        const MINIMUM_ALIGNMENT: usize = 16;

        let alignment = if layout.align().is_power_of_two()
            && (layout.align() & (MINIMUM_ALIGNMENT - 1)) == 0
        {
            layout.align()
        } else {
            warn!("Unsupported allocator alignment: {}", layout.align());
            warn!("Defaulting to alignment: 16");

            MINIMUM_ALIGNMENT
        };

        debug!(
            "Allocation requested: {} bytes, aligned by {}",
            layout.size(),
            alignment
        );

        let size_in_blocks = (layout.size() + (Self::BLOCK_SIZE - 1)) / Self::BLOCK_SIZE;
        let (mut block_index, mut current_run);

        while {
            block_index = 0;
            current_run = 0;

            'outer: for block_page in self.map.read().iter() {
                if block_page.is_full() {
                    current_run = 0;
                    block_index += BlockPage::BLOCK_COUNT;
                } else {
                    for section in block_page
                        .iter()
                        .map(|section| section.load(Ordering::Acquire))
                    {
                        if section == u64::MAX {
                            current_run = 0;
                            block_index += 64;
                        } else {
                            for bit in (0..64).map(|shift| (section & (1 << shift)) > 0) {
                                if bit {
                                    current_run = 0;
                                } else if current_run > 0
                                    || (current_run == 0 && (block_index % alignment) == 0)
                                {
                                    current_run += 1;
                                }

                                block_index += 1;

                                if current_run == size_in_blocks {
                                    break 'outer;
                                }
                            }
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
            "Allocating fulfilling: {}..{}",
            start_block_index,
            end_block_index
        );

        let start_map_index = start_block_index / BlockPage::BLOCK_COUNT;
        let mut initial_section_skip = crate::align_down_div(block_index, BlockPage::SECTION_LEN)
            - (start_map_index * BlockPage::SECTION_COUNT);

        for (map_index, block_page) in self
            .map
            .write()
            .iter_mut()
            .enumerate()
            .skip(start_map_index)
            .take(align_up_div(end_block_index, BlockPage::BLOCK_COUNT) - start_map_index)
        {
            let mut page_state: [SectionState; BlockPage::SECTION_COUNT] =
                [SectionState::empty(); BlockPage::SECTION_COUNT];

            for (section_index, section) in block_page.iter_mut().enumerate() {
                page_state[section_index].had_bits = section.load(Ordering::Acquire) > 0;

                if initial_section_skip > 0 {
                    initial_section_skip -= 1;
                } else if block_index < end_block_index {
                    let (bit_count, bit_mask) = Self::calculate_bit_fields(
                        map_index,
                        section_index,
                        end_block_index,
                        block_index,
                    );

                    assert_eq!(
                        section.load(Ordering::Acquire) & bit_mask,
                        0,
                        "attempting to allocate blocks that are already allocated"
                    );

                    section.fetch_or(bit_mask, Ordering::AcqRel);
                    block_index += bit_count;
                }

                page_state[section_index].has_bits = section.load(Ordering::Acquire) > 0;
            }

            info!("INDEX {}: {:?}", map_index, block_page);

            if SectionState::should_alloc(&page_state) {
                trace!("Allocating frame for previously unused block page.");

                // 'has bits', but not 'had bits'
                self.with_addressor(|addressor| {
                    let page = &mut Page::from_index(map_index);
                    addressor.map(page, &crate::memory::global_memory().lock_next().unwrap());
                    unsafe { page.clear() };
                });
            }

            // sanity check to ensure we're allocating block pages
            // assert!(
            //     self.is_mapped(Page::from_index(map_index).addr()),
            //     "previously iterated block page is unallocated at end of allocation step"
            // );
        }

        for (map_index, block_page) in self.map.write().iter().enumerate().skip(start_map_index) {
            info!("INDEX {}: {:?}", map_index, block_page);
        }

        (start_block_index * Self::BLOCK_SIZE) as *mut u8
    }

    fn raw_dealloc(&self, ptr: *mut u8, size: usize) {
        let start_block_index = (ptr as usize) / Self::BLOCK_SIZE;
        let end_block_index = start_block_index + align_up_div(size, Self::BLOCK_SIZE);
        let mut block_index = start_block_index;
        trace!(
            "Deallocating requested: {}..{}",
            start_block_index,
            end_block_index
        );

        let start_map_index = start_block_index / BlockPage::BLOCK_COUNT;
        let end_map_index = align_up_div(end_block_index, BlockPage::BLOCK_COUNT) - start_map_index;
        let mut initial_section_skip = crate::align_down_div(block_index, BlockPage::SECTION_LEN)
            - (start_map_index * BlockPage::SECTION_COUNT);
        for (map_index, block_page) in self
            .map
            .write()
            .iter_mut()
            .enumerate()
            .skip(start_map_index)
            .take(end_map_index)
        {
            let mut page_state: [SectionState; BlockPage::SECTION_COUNT] =
                [SectionState::empty(); BlockPage::SECTION_COUNT];

            for (section_index, section) in block_page.iter_mut().enumerate() {
                page_state[section_index].had_bits = section.load(Ordering::Acquire) > 0;

                if initial_section_skip > 0 {
                    initial_section_skip -= 1;
                } else if block_index < end_block_index {
                    let (bit_count, bit_mask) = Self::calculate_bit_fields(
                        map_index,
                        section_index,
                        end_block_index,
                        block_index,
                    );

                    assert_eq!(
                        section.load(Ordering::Acquire) & bit_mask,
                        bit_mask,
                        "attempting to deallocate blocks that are already deallocated"
                    );

                    section.fetch_xor(bit_mask, Ordering::AcqRel);
                    block_index += bit_count;
                }

                page_state[section_index].has_bits = section.load(Ordering::Acquire) > 0;
            }

            if SectionState::should_dealloc(&page_state) {
                // 'has bits', but not 'had bits'
                self.with_addressor(|addressor| {
                    let page = &Page::from_index(map_index);
                    // todo FIX THIS
                    //     unsafe {
                    //     crate::memory::global_memory()
                    //         .free_frame(&addressor.translate_page(page).unwrap())
                    //         .unwrap()
                    // };
                    addressor.unmap(page);
                });
            }
        }
    }

    /// Calculates the bit count and mask for a given set of block page parameters.
    fn calculate_bit_fields(
        map_index: usize,
        section_index: usize,
        end_block_index: usize,
        block_index: usize,
    ) -> (usize, u64) {
        let traversed_blocks =
            (map_index * BlockPage::BLOCK_COUNT) + (section_index * BlockPage::SECTION_LEN);
        let remaining_blocks = end_block_index - traversed_blocks;
        // Each block is one bit in our map, so we calculate the offset into
        //  the current section, at which our current index (`block_index`) lies.
        let bit_offset = block_index - traversed_blocks;
        let bit_count = core::cmp::min(BlockPage::SECTION_LEN, remaining_blocks) - bit_offset;
        // Finally, we acquire the respective bitmask to flip all relevant bits in
        //  our current section.
        (bit_count, Self::MASK_MAP[bit_count - 1] << bit_offset)
    }

    /// Allocates a region of memory pointing to the frame region indicated by
    ///  given the iterator.
    ///
    /// This function assumed the frames are already locked or otherwise valid.
    pub fn alloc_to(&self, mut frames: impl FrameIterator + Clone) -> *mut u8 {
        let size_in_frames = frames.clone().count();
        debug!("Allocation requested to: {} frames", size_in_frames);
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
            self.grow(size_in_frames * BlockPage::BLOCK_COUNT);
        }

        let start_index = map_index - current_run;
        trace!(
            "Allocation fulfilling: pages {}..{}",
            start_index,
            start_index + size_in_frames
        );

        self.with_addressor(|addressor| {
            for (map_index, block_page) in self
                .map
                .write()
                .iter_mut()
                .enumerate()
                .skip(start_index)
                .take(size_in_frames)
            {
                block_page.set_full();
                addressor.map(
                    &Page::from_index(map_index),
                    &frames.next().expect("invalid end of frame iterator"),
                );
            }
        });

        (start_index * 0x1000) as *mut u8
    }

    pub fn identity_map(&self, frame: &Frame, map: bool) {
        debug!("Identity mapping requested: {:?}", frame);

        let map_len = self.map.read().len();
        if map_len <= frame.index() {
            self.grow(((frame.index() - map_len) + 1) * BlockPage::BLOCK_COUNT);
        }

        let block_page = &mut self.map.write()[frame.index()];
        block_page.set_empty();
        assert!(
            block_page.is_empty(),
            "attempting to identity map page with previously allocated blocks: {:?} (map? {})\n {:?}",
            frame,
            map,
            block_page
        );
        block_page.set_full();

        if map {
            self.with_addressor(|addressor| addressor.identity_map(frame));
        }
    }

    pub fn grow(&self, required_blocks: usize) {
        assert!(required_blocks > 0, "calls to grow must be nonzero");

        trace!("Growing map to faciliate {} blocks.", required_blocks);
        const BLOCKS_PER_MAP_PAGE: usize = 8 /* bits per byte */ * 0x1000;
        let map_read = self.map.upgradeable_read();
        let cur_map_len = map_read.len();
        let cur_page_offset = (cur_map_len * BlockPage::BLOCK_COUNT) / BLOCKS_PER_MAP_PAGE;
        let new_page_offset =
            cur_page_offset + crate::align_up_div(required_blocks, BLOCKS_PER_MAP_PAGE);

        debug!(
            "Growing map: {}..{} pages",
            cur_page_offset, new_page_offset
        );

        self.with_addressor(|addressor| {
            for offset in cur_page_offset..new_page_offset {
                let map_page = &mut Self::ALLOCATOR_BASE.offset(offset);
                addressor.map(
                    map_page,
                    &crate::memory::global_memory().lock_next().unwrap(),
                );

                debug_assert!(
                    addressor.is_mapped(map_page.addr()),
                    "mapping allocator base offset page failed (offset {})",
                    offset
                );
            }

            const BLOCK_PAGES_PER_FRAME: usize = 0x1000 / size_of::<BlockPage>();
            let new_map_len = new_page_offset * BLOCK_PAGES_PER_FRAME;

            debug_assert_eq!(
                new_map_len % BLOCK_PAGES_PER_FRAME,
                0,
                "map must be page-aligned"
            );

            let mut map_write = map_read.upgrade();
            *map_write = unsafe {
                &mut *core::ptr::slice_from_raw_parts_mut(
                    Self::ALLOCATOR_BASE.mut_ptr(),
                    new_map_len,
                )
            };
            map_write[cur_map_len..].fill(BlockPage::empty());

            debug!(
                "Grew map: {} pages, {} block pages.",
                new_page_offset, new_map_len
            );
        });
    }

    pub fn translate_page(&self, page: &Page) -> Option<Frame> {
        self.with_addressor(|addressor| addressor.translate_page(page))
    }

    pub fn is_mapped(&self, virt_addr: x86_64::VirtAddr) -> bool {
        self.with_addressor(|addressor| addressor.is_mapped(virt_addr))
    }
}

unsafe impl core::alloc::GlobalAlloc for BlockAllocator<'_> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.raw_alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.raw_dealloc(ptr, layout.size());
    }
}
