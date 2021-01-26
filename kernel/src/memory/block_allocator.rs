use crate::memory::{paging::VirtualAddressorCell, Page};
use alloc::vec::Vec;
use spin::RwLock;
use x86_64::VirtAddr;

const ALLOCATOR_ADDRESS: VirtAddr = unsafe { VirtAddr::new_unsafe(0xA000000000) };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allocation {
    Arbitrary(usize),
    Fixed(VirtAddr, usize),
}

struct MemoryBlock {
    page: Page, // 8 bytes
                // 8 bytes free
}

pub struct BlockAllocator<'vaddr> {
    virtual_addressor: &'vaddr VirtualAddressorCell,
    blocks: RwLock<Vec<MemoryBlock>>,
}

impl BlockAllocator<'_> {
    // pub fn new(virtual_addressor: &'static VirtualAddressorCell) -> Self {
    //     if virtual_addressor.is_mapped(ALLOCATOR_ADDRESS) {
    //         panic!("allocator already exists for this virtual addressor (or allocator memory zone has been otherwise mapped)");
    //     } else {
    //         Self {
    //             virtual_addressor,
    //             // todo raw vec
    //             blocks: RwLock::new(),
    //         }
    //     }
    // }

    pub fn alloc(allocation: Allocation) {
        match allocation {
            Allocation::Arbitrary(size) => {}
            Allocation::Fixed(addr, size) => {}
        }
    }
}
