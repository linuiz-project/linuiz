use crate::{
    align_up_div,
    memory::{paging::VirtualAddressor, Frame, FrameIterator, Page},
    SYSTEM_SLICE_SIZE,
};
use alloc::vec::Vec;
use core::{
    mem::size_of,
    sync::atomic::{AtomicU64, Ordering},
};
use spin::{Mutex, RwLock};

/// Represents one page worth of memory blocks (i.e. 4096 bytes in blocks).
#[repr(C)]
#[derive(Debug)]
struct BlockPage {
    sections: [AtomicU64; 4],
}
const ATOMIC_U64_ZERO: AtomicU64 = AtomicU64::new(0);

impl BlockPage {
    /// Number of sections (primitive used to track blocks with its bits).
    const SECTION_COUNT: usize = 4;
    const SECTION_LEN: usize = size_of::<u64>() * 8;
    /// Number of blocks each block page contains.
    const BLOCK_COUNT: usize = Self::SECTION_COUNT * Self::SECTION_LEN;

    /// An empty block page (all blocks zeroed).
    const fn empty() -> Self {
        Self {
            sections: [ATOMIC_U64_ZERO; Self::SECTION_COUNT],
        }
    }

    /// Whether the block page is empty.
    pub fn is_empty(&self) -> bool {
        self.sections[0].load(Ordering::Acquire) == 0
            && self.sections[1].load(Ordering::Acquire) == 0
            && self.sections[2].load(Ordering::Acquire) == 0
            && self.sections[3].load(Ordering::Acquire) == 0
    }

    /// Whether the block page is full.
    pub fn is_full(&self) -> bool {
        self.sections[0].load(Ordering::Acquire) == u64::MAX
            && self.sections[1].load(Ordering::Acquire) == u64::MAX
            && self.sections[2].load(Ordering::Acquire) == u64::MAX
            && self.sections[3].load(Ordering::Acquire) == u64::MAX
    }

    /// Unset all of the block page's blocks.
    pub fn set_empty(&mut self) {
        self.sections[0].store(0, Ordering::Release);
        self.sections[1].store(0, Ordering::Release);
        self.sections[2].store(0, Ordering::Release);
        self.sections[3].store(0, Ordering::Release);
    }

    /// Set all of the block page's blocks.
    pub fn set_full(&mut self) {
        self.sections[0].store(u64::MAX, Ordering::Release);
        self.sections[1].store(u64::MAX, Ordering::Release);
        self.sections[2].store(u64::MAX, Ordering::Release);
        self.sections[3].store(u64::MAX, Ordering::Release);
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

impl Clone for BlockPage {
    fn clone(&self) -> Self {
        Self {
            sections: [
                AtomicU64::new(self.sections[0].load(Ordering::Acquire)),
                AtomicU64::new(self.sections[1].load(Ordering::Acquire)),
                AtomicU64::new(self.sections[2].load(Ordering::Acquire)),
                AtomicU64::new(self.sections[3].load(Ordering::Acquire)),
            ],
        }
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
pub struct BlockAllocator {
    addressor: Mutex<core::lazy::OnceCell<VirtualAddressor>>,
    map: RwLock<Vec<BlockPage>>,
}

impl BlockAllocator {
    /// The size of an allocator block.
    const BLOCK_SIZE: usize = 16;

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

    pub const fn new() -> Self {
        Self {
            addressor: Mutex::new(core::lazy::OnceCell::new()),
            map: RwLock::new(Vec::new()),
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

    pub fn init(&self, memory_map: &[crate::memory::UEFIMemoryDescriptor]) {
        debug!("Initializing global memory and mapping all frame allocator frames.");
        // TODO do this global memory init in a static / global context
        //  (allocators can't be considered global from the system's perspective)
        let global_memory_frames = unsafe { crate::memory::init_global_memory(memory_map) };

        unsafe {
            self.new_addressor();
            self.init_map();
        }

        let stack_descriptor = crate::memory::find_stack_descriptor(memory_map)
            .expect("failed to find stack memory region");

        self.with_addressor(|addressor| {
            debug!("Identity mapping global memory journal frames.");
            global_memory_frames.for_each(|frame| addressor.identity_map(&frame));

            debug!("Identity mapping all reserved memory blocks.");
            for frame in memory_map
                .iter()
                .filter(|descriptor| crate::memory::is_uefi_reserved_memory_type(descriptor.ty))
                .flat_map(|descriptor| {
                    Frame::range_count(descriptor.phys_start, descriptor.page_count as usize)
                })
            {
                addressor.identity_map(&frame);
            }

            debug!("Temporary identity mapping stack frames.");
            for frame in stack_descriptor.frame_iter() {
                // This is a temporary identity mapping, purely
                //  so `rsp` isn't invalid after we swap the PML4.
                addressor.identity_map(&frame);
                unsafe { crate::memory::global_reserve(&frame) };
            }

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

        // 'Allocate' the null page
        trace!("Allocating null frame.");
        self.identity_map(&Frame::null());

        debug!("Allocating global memory journaling frames.");
        global_memory_frames.for_each(|frame| self.identity_map(&frame));

        // 'Allocate' reserved memory
        trace!("Allocating reserved frames.");
        memory_map
            .iter()
            .filter(|descriptor| crate::memory::is_uefi_reserved_memory_type(descriptor.ty))
            .flat_map(|descriptor| {
                Frame::range_count(descriptor.phys_start, descriptor.page_count as usize)
            })
            .for_each(|frame| self.identity_map(&frame));

        self.alloc_stack_mapping(stack_descriptor);
    }

    unsafe fn new_addressor(&self) {
        if self
            .addressor
            .lock()
            .set(VirtualAddressor::new(Page::null()))
            .is_err()
        {
            panic!("addressor has already been set for allocator");
        }
    }

    unsafe fn init_map(&self) {
        *self.map.write() = Vec::from_raw_parts(
            Self::ALLOCATOR_BASE.mut_ptr(),
            0,
            SYSTEM_SLICE_SIZE / size_of::<BlockPage>(),
        );
    }

    fn alloc_stack_mapping(&self, stack_descriptor: &crate::memory::UEFIMemoryDescriptor) {
        stack_descriptor
            .frame_iter()
            .for_each(|frame| self.identity_map(&frame));

        // adjust the stack pointer to our new stack
        unsafe {
            let cur_stack_base = stack_descriptor.phys_start.as_u64();
            let stack_ptr = self.alloc_to(stack_descriptor.frame_iter()) as u64;

            if cur_stack_base > stack_ptr {
                crate::registers::stack::RSP::sub(cur_stack_base - stack_ptr);
            } else {
                crate::registers::stack::RSP::add(stack_ptr - cur_stack_base);
            }
        }

        // unmap the old stack mappings
        self.with_addressor(|addressor| {
            stack_descriptor
                .frame_iter()
                .for_each(|frame| addressor.unmap(&Page::from_index(frame.index())))
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

        trace!(
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
                    let traversed_blocks = (map_index * BlockPage::BLOCK_COUNT)
                        + (section_index * BlockPage::SECTION_LEN);
                    let bit_offset = block_index - traversed_blocks;
                    let bit_count =
                        core::cmp::min(BlockPage::SECTION_LEN, end_block_index - traversed_blocks)
                            - bit_offset;
                    let bit_mask = Self::MASK_MAP[bit_count - 1] << bit_offset;

                    assert_eq!(
                        section.load(Ordering::Acquire) & bit_mask,
                        0,
                        "attempting to allocate blocks that are already allocated"
                    );

                    unsafe { asm!("mov r11, cr3", "mov cr3, r11") };
                    info!("{} |= {}", section.load(Ordering::Acquire), bit_mask);
                    unsafe { asm!("mov r10, cr3", "mov cr3, r10") };
                    section.fetch_or(bit_mask, Ordering::AcqRel);
                    info!("== {}", section.load(Ordering::Acquire));
                    block_index += bit_count;
                }

                page_state[section_index].has_bits = section.load(Ordering::Acquire) > 0;
            }

            if SectionState::should_alloc(&page_state) {
                trace!("Allocating frame for previously unused block page.");

                // 'has bits', but not 'had bits'
                self.with_addressor(|addressor| {
                    let page = &mut Page::from_index(map_index);
                    addressor.map(page, unsafe { &crate::memory::global_lock_next().unwrap() });
                    unsafe { page.clear() };
                });
            }
        }

        (start_block_index * Self::BLOCK_SIZE) as *mut u8
    }

    pub fn alloc_to(&self, mut frames: FrameIterator) -> *mut u8 {
        trace!("Allocation requested to: {} frames", frames.remaining());
        let size_in_frames = frames.remaining();
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

    pub fn identity_map(&self, frame: &Frame) {
        trace!("Identity mapping requested: {:?}", frame);

        let map_len = self.map.read().len();
        if map_len <= frame.index() {
            self.grow(((frame.index() - map_len) + 1) * 0x8000);
        }

        self.with_addressor(|addressor| {
            let block_page = &mut self.map.write()[frame.index()];

            if block_page.is_empty() {
                block_page.set_full();
                addressor.identity_map(frame);
            } else {
                panic!("attempting to identity map page with previously allocated blocks");
            }
        });
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
                    let traversed_blocks = (map_index * BlockPage::BLOCK_COUNT)
                        + (section_index * BlockPage::SECTION_LEN);
                    let bit_offset = block_index - traversed_blocks;
                    let bit_count =
                        core::cmp::min(BlockPage::SECTION_LEN, end_block_index - traversed_blocks)
                            - bit_offset;
                    let bit_mask = Self::MASK_MAP[bit_count - 1] << bit_offset;

                    debug_assert_eq!(
                        section.load(Ordering::Acquire) & bit_mask,
                        bit_mask,
                        "attempting to allocate blocks that are already allocated"
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
                    unsafe { crate::memory::global_free(&addressor.translate_page(page).unwrap()) };
                    addressor.unmap(page);
                });
            }
        }
    }

    pub fn grow(&self, required_blocks: usize) {
        assert!(required_blocks > 0, "calls to grow much be nonzero");

        self.with_addressor(|addressor| {
            trace!("Growing map to faciliate {} blocks.", required_blocks);
            let map_read = self.map.upgradeable_read();
            let cur_map_blocks = map_read.len() * BlockPage::BLOCK_COUNT;
            let new_map_blocks = cur_map_blocks + crate::align_up(required_blocks, 0x8000);
            let cur_page_offset = cur_map_blocks / 0x8000;
            let new_page_offset = new_map_blocks / 0x8000;

            debug!(
                "Growing map: {}..{} pages",
                cur_page_offset, new_page_offset
            );
            for offset in cur_page_offset..new_page_offset {
                let page = &mut Self::ALLOCATOR_BASE.offset(offset);
                addressor.map(page, unsafe { &crate::memory::global_lock_next().unwrap() });

                assert!(addressor.is_mapped(page.addr()), "failed to map growth",);
            }

            const BLOCK_PAGES_PER_FRAME: usize = 0x1000 / size_of::<BlockPage>();
            let new_map_len = new_page_offset * BLOCK_PAGES_PER_FRAME;
            debug_assert_eq!(
                new_map_len % BLOCK_PAGES_PER_FRAME,
                0,
                "map must be page-aligned"
            );

            debug!("Grew map (new size: {} block pages).", new_map_len);
            let mut map_write = map_read.upgrade();
            map_write.resize(new_map_len, BlockPage::empty());
            trace!("Successfully grew allocator map: {:#?}.", map_write);
        });
    }

    pub fn translate_page(&self, page: &Page) -> Option<Frame> {
        self.with_addressor(|addressor| addressor.translate_page(page))
    }

    pub fn is_mapped(&self, virt_addr: x86_64::VirtAddr) -> bool {
        self.with_addressor(|addressor| addressor.is_mapped(virt_addr))
    }
}

unsafe impl core::alloc::GlobalAlloc for BlockAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.raw_alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.raw_dealloc(ptr, layout.size());
    }
}
