use crate::{
    io::pci::PCIDeviceHeader,
    memory::mmio::{Mapped, MMIO},
};

pub struct PCIeBus {
    mmio: MMIO<Mapped>,
}

impl PCIeBus {
    pub const fn new(mmio: MMIO<Mapped>) -> Self {
        Self { mmio }
    }

    pub fn header(&self) -> &PCIDeviceHeader {
        unsafe { self.mmio.read(0).unwrap() }
    }
}

impl core::fmt::Debug for PCIeBus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PCIe Bus")
            .field("Header", self.header())
            .finish()
    }
}
