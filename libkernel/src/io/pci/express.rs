use crate::{
    addr_ty::Physical,
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
    type Item = PCIEBus;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_bus < self.end_bus {
            let offset_addr = self.base_addr + ((self.cur_bus as usize) << 20);
            self.cur_bus += 1;
            let mmio_frames = unsafe {
                crate::memory::global_memory()
                    .acquire_frame(
                        offset_addr.as_usize() / 0x1000,
                        crate::memory::FrameState::MMIO,
                    )
                    .unwrap()
                    .into_iter()
            };

            let mmio = crate::memory::mmio::unmapped_mmio(mmio_frames)
                .unwrap()
                .map();
            Some(PCIEBus::new(mmio))
        } else {
            None
        }
    }
}
