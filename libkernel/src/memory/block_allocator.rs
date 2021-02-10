use crate::{
    memory::{paging::VirtualAddressor, Frame, FrameIterator, Page},
    SYSTEM_SLICE_SIZE,
};
use alloc::vec::Vec;
use spin::{Mutex, RwLock};

/// Represents one page worth of memory blocks (i.e. 4096 bytes in blocks).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct BlockPage {
    blocks: [u64; 4],
}

impl BlockPage {
    const SECTIONS_COUNT: usize = 4;
    const BLOCKS_COUNT: usize = Self::SECTIONS_COUNT * 64;

    const fn empty() -> Self {
        Self {
            blocks: [0u64; Self::SECTIONS_COUNT],
        }
    }

    pub const fn is_empty(&self) -> bool {
        (self.blocks[0] == 0)
            && (self.blocks[1] == 0)
            && (self.blocks[2] == 0)
            && (self.blocks[3] == 0)
    }

    pub const fn is_full(&self) -> bool {
        (self.blocks[0] == u64::MAX)
            && (self.blocks[1] == u64::MAX)
            && (self.blocks[2] == u64::MAX)
            && (self.blocks[3] == u64::MAX)
    }

    pub const fn set_empty(&mut self) {
        self.blocks[0] = 0;
        self.blocks[1] = 0;
        self.blocks[2] = 0;
        self.blocks[3] = 0;
    }

    pub const fn set_full(&mut self) {
        self.blocks[0] = u64::MAX;
        self.blocks[1] = u64::MAX;
        self.blocks[2] = u64::MAX;
        self.blocks[3] = u64::MAX;
    }

    fn iter(&self) -> core::slice::Iter<u64> {
        self.blocks.iter()
    }

    fn iter_mut(&mut self) -> core::slice::IterMut<u64> {
        self.blocks.iter_mut()
    }
}

#[derive(Debug, Clone, Copy)]
struct SectionState {
    had_bits: bool,
    has_bits: bool,
}

impl SectionState {
    const fn empty() -> Self {
        Self {
            had_bits: false,
            has_bits: false,
        }
    }

    const fn is_empty(&self) -> bool {
        !self.had_bits && !self.has_bits
    }

    const fn is_alloc(&self) -> bool {
        !self.had_bits && self.has_bits
    }

    const fn is_dealloc(&self) -> bool {
        self.had_bits && !self.has_bits
    }

    fn should_alloc(page_state: &[SectionState]) -> bool {
        page_state.iter().any(|state| state.is_alloc())
            && page_state
                .iter()
                .all(|state| state.is_alloc() || state.is_empty())
    }

    fn should_dealloc(page_state: &[SectionState]) -> bool {
        page_state.iter().any(|state| state.is_dealloc())
            && page_state
                .iter()
                .all(|state| state.is_dealloc() || state.is_empty())
    }
}

pub struct BlockAllocator {
    addressor: Mutex<core::lazy::OnceCell<VirtualAddressor>>,
    map: RwLock<Vec<BlockPage>>,
}

impl BlockAllocator {
    const BLOCK_SIZE: usize = 16;

    const ALLOCATOR_BASE: Page = Page::from_addr(x86_64::VirtAddr::new_truncate(
        (SYSTEM_SLICE_SIZE as u64) * 0xA,
    ));
    const ALLOCATOR_CAPACITY: usize = SYSTEM_SLICE_SIZE;

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

    pub unsafe fn init(&self, memory_map: &[crate::memory::UEFIMemoryDescriptor]) {
        debug!("Initializing global memory and mapping all frame allocator frames.");
        // TODO do this global memory init in a static / global context
        //  (allocators can't be considered global from the system's perspective)
        let frame_allocator_frames = unsafe { crate::memory::init_global_memory(memory_map) };

        if self
            .addressor
            .lock()
            .set(unsafe { VirtualAddressor::new(Page::null()) })
            .is_ok()
        {
            *self.map.write() = Vec::from_raw_parts(
                Self::ALLOCATOR_BASE.mut_ptr(),
                0,
                Self::ALLOCATOR_CAPACITY / core::mem::size_of::<BlockPage>(),
            );
        } else {
            panic!("addressor has already been set for allocator");
        }

        // We have to remap the stack.
        //
        // To make things fun, there's no pre-defined 'this is a stack'
        //  descriptor. So, as a work-around, we read `rsp`, and find the
        //  descriptor which contains it. I believe this is a flawless solution
        //  that has no possibility of backfiring.
        let rsp_addr = crate::registers::stack::RSP::read();
        // Still, this feels like I'm cheating on a math test
        let stack_descriptor = memory_map
            .iter()
            .find(|descriptor| descriptor.range().contains(&rsp_addr.as_u64()))
            .expect("failed to find stack memory region");

        if let Some(addressor) = self.addressor.lock().get_mut() {
            frame_allocator_frames.for_each(|frame| addressor.identity_map(&frame));

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
        } else {
            panic!("addressor has not been configured");
        }

        // 'Allocate' the null page
        trace!("Allocating null frame.");
        self.identity_map(&Frame::null());

        // 'Allocate' reserved memory
        trace!("Allocating reserved frames.");
        memory_map
            .iter()
            .filter(|descriptor| crate::memory::is_uefi_reserved_memory_type(descriptor.ty))
            .flat_map(|descriptor| {
                Frame::range_count(descriptor.phys_start, descriptor.page_count as usize)
            })
            .for_each(|frame| self.identity_map(&frame));

        stack_descriptor
            .frame_iter()
            .for_each(|frame| self.identity_map(&frame));
    }

    /* ALLOC & DEALLOC */

    fn raw_alloc(&self, size: usize) -> *mut u8 {
        trace!("Allocation requested: {} bytes", size);

        let size_in_blocks = (size + (Self::BLOCK_SIZE - 1)) / Self::BLOCK_SIZE;
        let (mut block_index, mut current_run);

        while {
            block_index = 0;
            current_run = 0;

            'outer: for block_page in self.map.read().iter() {
                if block_page.is_full() {
                    current_run = 0;
                    block_index += BlockPage::BLOCKS_COUNT;
                } else {
                    for block_section in block_page.iter().map(|section| *section) {
                        if block_section == u64::MAX {
                            current_run = 0;
                            block_index += 64;
                        } else {
                            for bit in (0..64).map(|shift| (block_section & (1 << shift)) > 0) {
                                if bit {
                                    current_run = 0;
                                } else {
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

        let start_map_index = start_block_index / BlockPage::BLOCKS_COUNT;
        for (map_index, block_page) in self
            .map
            .write()
            .iter_mut()
            .enumerate()
            .skip(start_map_index)
            .take(crate::align_up_div(end_block_index, BlockPage::BLOCKS_COUNT) - start_map_index)
        {
            let mut page_state: [SectionState; 4] = [SectionState::empty(); 4];

            for (section_index, section) in block_page.iter_mut().enumerate() {
                page_state[section_index].had_bits = *section > 0;

                if block_index < end_block_index {
                    let traversed_blocks =
                        (map_index * BlockPage::BLOCKS_COUNT) + (section_index * 64);
                    let start_byte_bits = block_index - traversed_blocks;
                    let total_bits =
                        core::cmp::min(64, end_block_index - traversed_blocks) - start_byte_bits;
                    let bits_mask = Self::MASK_MAP[total_bits - 1] << start_byte_bits;

                    debug_assert_eq!(
                        *section & bits_mask,
                        0,
                        "attempting to allocate blocks that are already allocated"
                    );

                    *section |= bits_mask;
                    block_index += total_bits;
                }

                page_state[section_index].has_bits = *section > 0;
            }

            if SectionState::should_alloc(&page_state) {
                // 'has bits', but not 'had bits'
                if let Some(addressor) = self.addressor.lock().get_mut() {
                    addressor.map(&Page::from_index(map_index), unsafe {
                        &crate::memory::global_lock_next().unwrap()
                    });
                } else {
                    panic!("addressor has not been configured");
                }
            }
        }

        (start_block_index * Self::BLOCK_SIZE) as *mut u8
    }

    pub fn alloc_to(&self, mut frames: FrameIterator) -> *mut u8 {
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
            }

            current_run < frames.remaining()
        } {
            self.grow(frames.remaining() * BlockPage::BLOCKS_COUNT);
        }

        let start_index = map_index - current_run;
        if let Some(addressor) = self.addressor.lock().get_mut() {
            for (map_index, block_page) in self
                .map
                .write()
                .iter_mut()
                .enumerate()
                .skip(start_index)
                .take(frames.remaining())
            {
                addressor.map(
                    &Page::from_index(map_index),
                    &frames.next().expect("invalid end of frame iterator"),
                );
                block_page.set_full();
            }
        } else {
            panic!("addressor has not been configured");
        }

        (start_index * 0x1000) as *mut u8
    }

    pub fn identity_map(&self, frame: &Frame) {
        trace!("Identity mapping requested: {:?}", frame);

        let map_len = self.map.read().len();
        if map_len <= frame.index() {
            self.grow((frame.index() - map_len) * BlockPage::BLOCKS_COUNT)
        }

        if let Some(addressor) = self.addressor.lock().get_mut() {
            let block_page = &mut self.map.write()[frame.index()];

            if block_page.is_empty() {
                block_page.set_full();
                addressor.identity_map(frame);
            } else {
                panic!("attempting to identity map page with previously allocated blocks");
            }
        } else {
            panic!("addressor has not been configured");
        }
    }

    fn raw_dealloc(&self, _ptr: *mut u8, _size: usize) {
        //     let start_block_index =
        //         ((ptr as usize) - (self.alloc_page.addr().as_u64() as usize)) / Self::BLOCK_SIZE;
        //     let end_block_index =
        //         start_block_index + ((size + (Self::BLOCK_SIZE - 1)) / Self::BLOCK_SIZE);
        //     let mut block_index = start_block_index;
        //     trace!(
        //         "Deallocating blocks: {}..{}",
        //         start_block_index,
        //         end_block_index
        //     );

        //     let start_map_index = start_block_index / 8;
        //     for (traversed_blocks, byte) in self
        //         .map
        //         .write()
        //         .iter_mut()
        //         .enumerate()
        //         .skip(start_map_index)
        //         .take(((end_block_index + 7) / 8) - start_map_index)
        //         .map(|(map_index, byte)| (map_index * 8, byte))
        //     {
        //         let start_byte_bit = block_index - traversed_blocks;
        //         let total_bits = core::cmp::min(8, end_block_index - traversed_blocks) - start_byte_bit;
        //         let value = Self::MASK_MAP[total_bits - 1] << start_byte_bit;

        //         debug_assert_eq!(
        //             *byte & value,
        //             value,
        //             "attempting to deallocate blocks that are aren't allocated"
        //         );

        //         *byte ^= value;
        //         block_index += total_bits;
        //     }
    }

    pub fn grow(&self, required_blocks: usize) {
        if let Some(addressor) = self.addressor.lock().get_mut() {
            let map_read = self.map.upgradeable_read();
            let new_map_len = usize::next_power_of_two(
                (map_read.len() * BlockPage::BLOCKS_COUNT) + required_blocks,
            );

            use core::mem::size_of;
            let frame_usage = ((map_read.len() * size_of::<BlockPage>()) + 0xFFF) / 0x1000;
            let new_frame_usage = ((new_map_len * size_of::<BlockPage>()) + 0xFFF) / 0x1000;
            trace!("Growth frame usage: {} -> {}", frame_usage, new_frame_usage);
            for offset in frame_usage..new_frame_usage {
                addressor.map(&Self::ALLOCATOR_BASE.offset(offset), unsafe {
                    &crate::memory::global_lock_next().unwrap()
                });
            }

            map_read.upgrade().resize(new_map_len, BlockPage::empty());
            trace!("Successfully grew allocator map.");
        } else {
            panic!("addressor has not been configured.");
        }
    }
}

unsafe impl core::alloc::GlobalAlloc for BlockAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.raw_alloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.raw_dealloc(ptr, layout.size());
    }
}
