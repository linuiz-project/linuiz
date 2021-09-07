use crate::{
    addr_ty::{Physical, Virtual},
    memory::FrameIterator,
    volatile::Volatile,
    Address, ReadOnly, ReadWrite,
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
    fn max_offset(&self) -> usize {
        self.frames.len() * 0x1000
    }

    unsafe fn mapped_offset<T>(&self, offset: usize) -> Result<*mut T, MMIOError> {
        if offset < self.max_offset() {
            Ok((self.mapped_addr() + offset).as_ptr::<T>() as *mut T)
        } else {
            Err(MMIOError::OffsetOverrun(offset, self.max_offset()))
        }
    }

    pub unsafe fn write<T>(&self, add_offset: usize, value: T) -> Result<(), MMIOError> {
        match self.mapped_offset::<T>(add_offset) {
            Ok(ptr) => {
                ptr.write_volatile(value);
                Ok(())
            }
            Err(mmio_err) => Err(mmio_err),
        }
    }

    pub unsafe fn read<T>(&self, add_offset: usize) -> Result<Volatile<T, ReadOnly>, MMIOError> {
        self.mapped_offset::<T>(add_offset)
            .map(|ptr| Volatile::<T, ReadOnly>::new(ptr))
    }

    pub unsafe fn read_mut<T>(&self, offset: usize) -> Result<Volatile<T, ReadWrite>, MMIOError> {
        self.mapped_offset::<T>(offset)
            .map(|ptr| Volatile::<T, ReadWrite>::new(ptr))
    }

    pub fn physical_addr(&self) -> Address<Physical> {
        self.frames.start().addr()
    }

    pub fn mapped_addr(&self) -> Address<Virtual> {
        self.mapped_addr
    }

    pub fn pages(&self) -> super::PageIterator {
        let base_page = Page::from_addr(self.mapped_addr());
        super::PageIterator::new(&base_page, &base_page.offset(self.frames.captured_len()))
    }
}

pub fn unmapped_mmio(frames: FrameIterator) -> Result<MMIO<Unmapped>, MMIOError> {
    Ok(MMIO::<Unmapped> {
        frames,
        mapped_addr: Address::zero(),
        phantom: core::marker::PhantomData,
    })
}
