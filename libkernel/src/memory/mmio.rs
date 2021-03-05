use crate::memory::FrameIterator;
use x86_64::VirtAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MMIOError {
    OffsetOverrun,
}

pub trait MMIOState {}

pub enum Unmapped {}
impl MMIOState for Unmapped {}

pub enum Mapped {}
impl MMIOState for Mapped {}

pub struct MMIO<S: MMIOState> {
    frames: FrameIterator,
    mapped_addr: VirtAddr,
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
    /// Internally maps the MMIO to the given virtual address.
    ///
    /// Safety: This function assumes the given mapped address is a valid page table mapping.
    pub unsafe fn unsafe_map(self, mapped_addr: x86_64::VirtAddr) -> MMIO<Mapped> {
        MMIO::<Mapped> {
            frames: self.frames,
            mapped_addr,
            phantom: core::marker::PhantomData,
        }
    }

    #[cfg(feature = "kernel_impls")]
    pub fn map(self) -> MMIO<Mapped> {
        let mapped_addr = VirtAddr::from_ptr::<u8>(crate::alloc_to!(&self.frames));

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

    unsafe fn mapped_offset<T>(&self, offset: usize) -> Result<*const T, MMIOError> {
        if offset < self.max_offset() {
            Ok((self.mapped_addr + offset).as_ptr())
        } else {
            Err(MMIOError::OffsetOverrun)
        }
    }

    unsafe fn mapped_offset_mut<T>(&mut self, offset: usize) -> Result<*mut T, MMIOError> {
        if offset < self.max_offset() {
            Ok((self.mapped_addr + offset).as_mut_ptr())
        } else {
            Err(MMIOError::OffsetOverrun)
        }
    }

    pub unsafe fn write<T>(&mut self, offset: usize, value: T) -> Result<(), MMIOError> {
        match self.mapped_offset_mut::<T>(offset) {
            Ok(ptr) => {
                ptr.write(value);
                Ok(())
            }
            Err(mmio_err) => Err(mmio_err),
        }
    }

    pub unsafe fn read<T>(&self, offset: usize) -> Result<&T, MMIOError> {
        match self.mapped_offset::<T>(offset) {
            Ok(ptr) => Ok(&*ptr),
            Err(mmio_err) => Err(mmio_err),
        }
    }

    pub unsafe fn read_mut<T>(&mut self, offset: usize) -> Result<&mut T, MMIOError> {
        match self.mapped_offset_mut::<T>(offset) {
            Ok(ptr) => Ok(&mut *ptr),
            Err(mmio_err) => Err(mmio_err),
        }
    }
}

pub fn unmapped_mmio(frames: FrameIterator) -> Result<MMIO<Unmapped>, MMIOError> {
    Ok(MMIO::<Unmapped> {
        frames,
        mapped_addr: VirtAddr::zero(),
        phantom: core::marker::PhantomData,
    })
}
