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
    const fn new(mmio: MMIO<Mapped>) -> Self {
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

pub struct PCIEDeviceIterator {
    base_addr: Address<Physical>,
    cur_bus: u8,
    end_bus: u8,
}

impl PCIEDeviceIterator {
    pub const fn new(base_addr: Address<Physical>, start_bus: u8, end_bus: u8) -> Self {
        Self {
            base_addr,
            cur_bus: start_bus,
            end_bus,
        }
    }
}

impl Iterator for PCIEDeviceIterator {
    type Item = Option<PCIEBus>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_bus < self.end_bus {
            let offset_addr = self.base_addr + ((self.cur_bus as usize) << 20);
            self.cur_bus += 1;
            let mmio_frames = unsafe {
                crate::memory::falloc::get()
                    .acquire_frame(
                        offset_addr.as_usize() / 0x1000,
                        crate::memory::falloc::FrameState::MMIO,
                    )
                    .unwrap()
                    .into_iter()
            };

            let pci_bus = PCIEBus::new(
                crate::memory::mmio::unmapped_mmio(mmio_frames)
                    .unwrap()
                    .map(),
            );

            Some(if !pci_bus.header().is_invalid() {
                Some(pci_bus)
            } else {
                None
            })
        } else {
            None
        }
    }
}
