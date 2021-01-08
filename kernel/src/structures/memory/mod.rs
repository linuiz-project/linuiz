pub mod paging;

use x86_64::{PhysAddr, VirtAddr};

pub const PAGE_SIZE: usize = 0x1000; // 4096
pub const KIBIBYTE: usize = 0x400; // 1024
pub const MIBIBYTE: usize = KIBIBYTE * KIBIBYTE;

pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame>;
    fn deallocate_frame(&mut self, frame: Frame);
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    index: u64,
}

impl Frame {
    pub const fn trim_addr(addr: u64) -> u64 {
        addr & 0x000FFFFF_FFFFF000000
    }

    pub const fn new(addr: u64) -> Self {
        Self {
            index: addr / (PAGE_SIZE as u64),
        }
    }

    pub const fn from_virt(addr: VirtAddr) -> Self {
        Self {
            index: addr.as_u64() / (PAGE_SIZE as u64),
        }
    }

    pub const fn from_phys(addr: PhysAddr) -> Self {
        Self {
            index: addr.as_u64() / (PAGE_SIZE as u64),
        }
    }

    pub const unsafe fn from_index(index: u64) -> Self {
        Self { index }
    }

    pub fn addr(&self) -> PhysAddr {
        PhysAddr::new(self.index * (PAGE_SIZE as u64))
    }

    pub unsafe fn clear(&mut self) {
        core::ptr::write_bytes(
            (self.addr().as_u64()) as *mut usize,
            0x0,
            PAGE_SIZE / core::mem::size_of::<usize>(),
        );
    }
}

pub fn to_kibibytes(value: usize) -> usize {
    value / KIBIBYTE
}

pub fn to_mibibytes(value: usize) -> usize {
    value / MIBIBYTE
}
