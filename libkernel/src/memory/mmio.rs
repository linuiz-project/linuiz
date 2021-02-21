use x86_64::{PhysAddr, VirtAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MMIOError {
    InsideSystemRAM,
    OffsetOverrun,
}

pub struct MMIO {
    base_addr: PhysAddr,
    mapped_addr: VirtAddr,
    size: usize,
}

impl MMIO {
    pub unsafe fn new(
        base_addr: PhysAddr,
        mapped_addr: VirtAddr,
        size: usize,
    ) -> Result<Self, MMIOError> {
        if (base_addr.as_u64() as usize) >= crate::memory::global_memory().total_memory() {
            Ok(Self {
                base_addr,
                mapped_addr,
                size,
            })
        } else {
            Err(MMIOError::InsideSystemRAM)
        }
    }

    pub fn base_addr(&self) -> PhysAddr {
        self.base_addr
    }

    unsafe fn mapped_offset<T>(&self, offset: usize) -> Result<*const T, MMIOError> {
        if offset < self.size {
            Ok((self.mapped_addr + (offset as u64)).as_ptr())
        } else {
            Err(MMIOError::OffsetOverrun)
        }
    }

    unsafe fn mapped_offset_mut<T>(&mut self, offset: usize) -> Result<*mut T, MMIOError> {
        if offset < self.size {
            Ok((self.mapped_addr + (offset as u64)).as_mut_ptr())
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
