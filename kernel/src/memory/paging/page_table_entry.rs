use crate::memory::{allocators::global_memory_mut, Frame};
use bitflags::bitflags;
use x86_64::PhysAddr;

bitflags! {
    pub struct PageAttributes : u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER_ACCESSIBLE = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const DISABLE_CACHE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        // 3 bits free for use by OS
        const NO_EXECUTE = 1 << 63;
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn unused() -> Self {
        Self { 0: 0 }
    }

    pub fn attribs(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.0)
    }

    pub fn frame(&self) -> Option<Frame> {
        if self.is_present() {
            Some(Frame::from_addr(PhysAddr::new(
                self.0 & 0x000FFFFF_FFFFF000,
            )))
        } else {
            None
        }
    }

    pub fn frame_create(&mut self) -> Frame {
        self.frame().unwrap_or_else(|| {
            trace!("Allocating frame for previously nonpresent entry.");
            let alloc_frame = global_memory_mut(|allocator| {
                allocator
                    .lock_next()
                    .expect("failed to allocate a frame for new page table")
            });

            self.set(
                &alloc_frame,
                PageAttributes::PRESENT | PageAttributes::WRITABLE,
            );

            alloc_frame
        })
    }

    pub fn set(&mut self, frame: &Frame, attribs: PageAttributes) {
        self.0 = frame.addr().as_u64() | attribs.bits();
    }

    pub fn is_present(&self) -> bool {
        self.attribs().contains(PageAttributes::PRESENT)
    }

    pub fn set_nonpresent(&mut self) {
        self.0 &= !PageAttributes::PRESENT.bits();
    }
}

impl core::fmt::Debug for PageTableEntry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("PageDescriptor")
            .field(&self.frame())
            .field(&self.attribs())
            .finish()
    }
}
