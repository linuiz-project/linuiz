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
    addr: usize,
    size: usize,
    phantom: core::marker::PhantomData<S>,
}

pub fn unmapped_mmio(
    phys_addr: x86_64::PhysAddr,
    size: usize,
) -> Result<MMIO<Unmapped>, MMIOError> {
    Ok(MMIO::<Unmapped> {
        addr: phys_addr.as_u64() as usize,
        size,
        phantom: core::marker::PhantomData,
    })
}

impl<S: MMIOState> MMIO<S> {
    pub fn size(&self) -> usize {
        self.size
    }
}

impl MMIO<Unmapped> {
    pub unsafe fn map(self, mapped_addr: x86_64::VirtAddr) -> MMIO<Mapped> {
        MMIO::<Mapped> {
            addr: mapped_addr.as_u64() as usize,
            size: self.size,
            phantom: core::marker::PhantomData,
        }
    }
}

impl MMIO<Mapped> {
    unsafe fn mapped_offset<T>(&self, offset: usize) -> Result<*const T, MMIOError> {
        if offset < self.size {
            Ok((self.addr + offset) as *const T)
        } else {
            Err(MMIOError::OffsetOverrun)
        }
    }

    unsafe fn mapped_offset_mut<T>(&mut self, offset: usize) -> Result<*mut T, MMIOError> {
        if offset < self.size {
            Ok((self.addr + offset) as *mut T)
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
