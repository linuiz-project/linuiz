use crate::{
    addr_ty::{Physical, Virtual},
    memory::FrameIterator,
    Address,
};

use super::Page;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MMIOError {
    OffsetOverrun(usize, usize),
}

pub trait MMIOState {}

pub enum Unmapped {}
impl MMIOState for Unmapped {}

pub enum Mapped {}
impl MMIOState for Mapped {}

pub struct MMIO<S: MMIOState> {
    frames: FrameIterator,
    mapped_addr: Address<Virtual>,
    phantom: core::marker::PhantomData<S>,
}

impl<S: MMIOState> MMIO<S> {
    pub fn frames(&self) -> &FrameIterator {
        &self.frames
    }
}

impl<S: MMIOState> core::fmt::Debug for MMIO<S> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MMIO")
            .field("Frames", &self.frames)
            .field("Mapped Address", &self.mapped_addr)
            .finish()
    }
}

impl MMIO<Unmapped> {
    pub fn automap(self) -> MMIO<Mapped> {
        let mapped_addr = Address::from_ptr::<u8>(crate::alloc_to!(&self.frames));

        // Ensure we mark MMIO page table entries as UC
        for page_index in
            mapped_addr.page_index()..(mapped_addr.page_index() + self.frames.total_len())
        {
            unsafe {
                use crate::memory::paging::{AttributeModify, PageAttributes};
                crate::memory::malloc::get().set_page_attributes(
                    &Page::from_index(page_index),
                    PageAttributes::UNCACHEABLE,
                    AttributeModify::Insert,
                );
            }
        }

        MMIO::<Mapped> {
            frames: self.frames,
            mapped_addr,
            phantom: core::marker::PhantomData,
        }
    }

    pub unsafe fn map(self, mapped_addr: Address<Virtual>) -> MMIO<Mapped> {
        MMIO::<Mapped> {
            frames: self.frames,
            mapped_addr,
            phantom: core::marker::PhantomData,
        }
    }
}

impl MMIO<Mapped> {
    const fn max_offset(&self) -> usize {
        self.frames.total_len() * 0x1000
    }

    pub fn physical_addr(&self) -> Address<Physical> {
        self.frames.start().base_addr()
    }

    pub const fn mapped_addr(&self) -> Address<Virtual> {
        self.mapped_addr
    }

    pub const fn mapped_offset<T>(&self, add_offset: usize) -> Result<*mut T, MMIOError> {
        if add_offset < self.max_offset() {
            Ok((self.mapped_addr().as_usize() + add_offset) as *mut T)
        } else {
            Err(MMIOError::OffsetOverrun(add_offset, self.max_offset()))
        }
    }

    pub fn pages(&self) -> super::PageIterator {
        let base_page = Page::from_addr(self.mapped_addr());
        super::PageIterator::new(
            &base_page,
            &base_page.forward(self.frames.total_len()).unwrap(),
        )
    }

    pub unsafe fn read<T>(&self, offset: usize) -> Result<T, MMIOError> {
        self.mapped_offset::<T>(offset)
            .map(|ptr| ptr.read_volatile())
    }

    pub unsafe fn write<T>(&self, offset: usize, value: T) -> Result<(), MMIOError> {
        self.mapped_offset::<T>(offset)
            .map(|ptr| ptr.write_volatile(value))
    }

    pub unsafe fn borrow<T: super::volatile::Volatile>(
        &self,
        add_offset: usize,
    ) -> Result<&T, MMIOError> {
        self.mapped_offset(add_offset).map(|ptr| &*ptr)
    }
}

pub fn unmapped_mmio(frames: FrameIterator) -> Result<MMIO<Unmapped>, MMIOError> {
    Ok(MMIO::<Unmapped> {
        frames,
        mapped_addr: Address::zero(),
        phantom: core::marker::PhantomData,
    })
}
