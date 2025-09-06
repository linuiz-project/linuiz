mod buddy;
use core::ptr::NonNull;

use buddy::Buddy;

mod bitmap;
use bitmap::BitMap;

use crate::{mem::HigherHalfDirectMap, util::sync::Once};
use libsys::{
    address::{Address, Frame},
    constants::{huge_page_size, large_page_size, page_size},
};

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("attempted to index out of bounds")]
    OutOfBounds,
}

#[derive(Debug, Clone, Copy)]
pub enum PageSize {
    Standard,
    Large,
    Huge,
}

impl PageSize {
    pub fn size_in_bytes(self) -> usize {
        match self {
            PageSize::Standard => page_size(),
            PageSize::Large => large_page_size(),
            PageSize::Huge => huge_page_size(),
        }
    }
}

trait PhysicalMemoryManagerKind {
    fn total_frames(&self) -> usize;

    fn total_memory(&self) -> usize {
        self.total_frames() * libsys::constants::page_size()
    }

    fn next_free_frame(&self, page_size: PageSize) -> Option<Address<Frame>>;

    fn lock_frame(&self, address: Address<Frame>) -> Result<(), FrameError>;
    fn free_frame(&self, address: Address<Frame>) -> Result<(), FrameError>;
    fn is_locked(&self, address: Address<Frame>) -> Result<bool, FrameError>;
}

enum Kind<'a> {
    Bitmap(BitMap<'a>),
    Buddy(Buddy<'a>),
}

pub struct PhysicalMemoryManager<'a> {
    kind: Kind<'a>,
}

static PHYSICAL_MEMORY_MANAGER: Once<PhysicalMemoryManager> = Once::new();

// Safety: `PhysicalMemoryManager` uses interrupt-safe synchronization.
unsafe impl Send for PhysicalMemoryManager<'_> {}

impl<'a: 'static> PhysicalMemoryManager<'a> {
    /// Initializes the static physical memory manager with the provided
    /// bootloader memory map request.
    pub fn init(memory_map_request: &limine::request::MemoryMapRequest) {
        PHYSICAL_MEMORY_MANAGER.call_once(|| todo!());
    }

    fn get_static() -> &'a Kind<'a> {
        PHYSICAL_MEMORY_MANAGER.get().map(|pmm| &pmm.kind).unwrap()
    }

    pub fn total_frames() -> usize {
        match Self::get_static() {
            Kind::Bitmap(bitmap) => bitmap.total_frames(),
            Kind::Buddy(buddy) => buddy.total_frames(),
        }
    }

    pub fn total_memory() -> usize {
        match Self::get_static() {
            Kind::Bitmap(bitmap) => bitmap.total_frames(),
            Kind::Buddy(buddy) => buddy.total_frames(),
        }
    }

    pub fn next_free_frame(page_size: PageSize, clear_memory: bool) -> Option<Address<Frame>> {
        let frame = match Self::get_static() {
            Kind::Bitmap(bit_map) => bit_map.next_free_frame(page_size)?,
            Kind::Buddy(buddy) => buddy.next_free_frame(page_size)?,
        };

        if clear_memory {
            let address = HigherHalfDirectMap::offset(frame.get().get());
            let ptr = NonNull::<u8>::with_exposed_provenance(address);

                // Safety:
            unsafe {
                NonNull::write_bytes(ptr, 0, page_size.size_in_bytes());
            }
        }

        Some(frame)
    }

    pub fn lock_frame(address: Address<Frame>) -> Result<(), FrameError> {
        match Self::get_static() {
            Kind::Bitmap(bitmap) => bitmap.lock_frame(address),
            Kind::Buddy(buddy) => buddy.lock_frame(address),
        }
    }

    pub fn free_frame(address: Address<Frame>) -> Result<(), FrameError> {
        match Self::get_static() {
            Kind::Bitmap(bitmap) => bitmap.free_frame(address),
            Kind::Buddy(buddy) => buddy.free_frame(address),
        }
    }

    pub fn is_locked(address: Address<Frame>) -> Result<bool, FrameError> {
        match Self::get_static() {
            Kind::Bitmap(bitmap) => bitmap.is_locked(address),
            Kind::Buddy(buddy) => buddy.is_locked(address),
        }
    }
}
