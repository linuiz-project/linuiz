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
        if (base_addr.as_u64() as usize) >= crate::memory::global_total() {
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

    fn mapped_offset<T>(&self, offset: usize) -> Result<*const T, MMIOError> {
        if offset < self.size {
            Ok(unsafe { self.mapped_addr.as_ptr::<u8>().offset(offset as isize) as *const T })
        } else {
            Err(MMIOError::OffsetOverrun)
        }
    }

    fn mapped_offset_mut<T>(&mut self, offset: usize) -> Result<*mut T, MMIOError> {
        if offset < self.size {
            Ok(unsafe { self.mapped_addr.as_mut_ptr::<u8>().offset(offset as isize) as *mut T })
        } else {
            Err(MMIOError::OffsetOverrun)
        }
    }

    pub fn write<T>(&mut self, offset: usize, value: T) -> Option<MMIOError> {
        unsafe {
            match self.mapped_offset_mut::<T>(offset) {
                Ok(ptr) => {
                    ptr.write(value);
                    None
                }
                Err(mmio_err) => Some(mmio_err),
            }
        }
    }

    pub fn read<T>(&self, offset: usize) -> Result<&T, MMIOError> {
        unsafe {
            match self.mapped_offset::<T>(offset) {
                Ok(ptr) => Ok(&*ptr),
                Err(mmio_err) => Err(mmio_err),
            }
        }
    }

    pub fn read_mut<T>(&mut self, offset: usize) -> Result<&mut T, MMIOError> {
        unsafe {
            match self.mapped_offset_mut::<T>(offset) {
                Ok(ptr) => Ok(&mut *ptr),
                Err(mmio_err) => Err(mmio_err),
            }
        }
    }
}
