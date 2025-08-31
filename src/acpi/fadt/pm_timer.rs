use core::ptr::NonNull;
use ioports::ReadOnlyPort;

#[derive(Debug)]
pub enum Source {
    PortIo(ReadOnlyPort<u32>),
    MemoryIo(NonNull<u32>),
}

#[derive(Debug)]
pub struct PmTimer {
    source: Source,
    is_32_bit: bool,
}

impl PmTimer {
    /// # Safety
    ///
    /// - `source` must the ACPI power management timer source address space
    ///   (either port or memory IO) that is implemented by hardware and
    ///   firmware on the current machine.
    /// - `source` must be the correct address to read the 32 or 24 -bit timer
    ///   value.
    pub unsafe fn new(source: Source, is_32_bit: bool) -> Self {
        Self { source, is_32_bit }
    }

    pub fn read(&self) -> u64 {
        match &self.source {
            Source::PortIo(address) => u64::from(address.read()),
            Source::MemoryIo(address) => {
                // Safety: `Self::new` requires `address` be valid as 32-bit MMIO.
                let value = unsafe { address.read_volatile() };
                u64::from(value)
            }
        }
    }

    pub fn max_value(&self) -> u64 {
        if self.is_32_bit {
            0xFFFF_FFFF
        } else {
            0x00FF_FFFF
        }
    }
}
