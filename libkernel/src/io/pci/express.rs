use crate::{
    io::pci::{PCIDeviceHeader, PCIeDeviceType},
    memory::mmio::{Mapped, MMIO},
};

pub struct PCIeBus {
    mmio: MMIO<Mapped>,
}

impl PCIeBus {
    pub const fn new(mmio: MMIO<Mapped>) -> Self {
        Self { mmio }
    }

    pub fn base_header(&self) -> &PCIDeviceHeader {
        unsafe { self.mmio.read(0).unwrap() }
    }

    pub fn ext_header(&self) -> PCIeDeviceType {
        match self.base_header().header_type() {
            0x0 => PCIeDeviceType::Standard(unsafe { self.mmio.read(0).unwrap() }),
            0x1 => PCIeDeviceType::PCI2PCI(unsafe { self.mmio.read(0).unwrap() }),
            0x2 => PCIeDeviceType::PCI2CardBus(unsafe { self.mmio.read(0).unwrap() }),
            header_type => panic!("invalid header type: 0x{:X}", header_type),
        }
    }
}

impl core::fmt::Debug for PCIeBus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PCIe Bus")
            .field("Header", self.base_header())
            .field("Extended Header", &self.ext_header())
            .finish()
    }
}
