use crate::{
    addr_ty::Physical,
    io::pci::PCIDeviceHeader,
    memory::mmio::{Mapped, MMIO},
    Address,
};
use core::u8;

pub struct PCIEBus {
    mmio: MMIO<Mapped>,
}

impl PCIEBus {
    pub const fn new(mmio: MMIO<Mapped>) -> Self {
        Self { mmio }
    }

    pub fn header(&self) -> &PCIDeviceHeader {
        unsafe { self.mmio.read(0).unwrap() }
    }
}

impl core::fmt::Debug for PCIEBus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PCIe Bus")
            .field("Header", self.header())
            .finish()
    }
}
