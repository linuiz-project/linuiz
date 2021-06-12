pub mod hba;

use hba::HostBustAdapterMemory;
use libkernel::{
    io::pci::{PCIeDevice, Standard},
    memory::mmio::{Mapped, MMIO},
};
use spin::MutexGuard;

pub struct AHCI<'dev> {
    device: &'dev PCIeDevice<Standard>,
    hba_memory: MutexGuard<'dev, MMIO<Mapped>>,
}

impl<'dev> AHCI<'dev> {
    pub fn from_pcie_device(device: &'dev PCIeDevice<Standard>) -> Self {
        trace!("Using PCIe device for AHCI driver:\n{:#?}", device);

        info!("{:?}", device.reg0());
        info!("{:?}", device.reg1());
        info!("{:?}", device.reg2());
        info!("{:?}", device.reg3());
        info!("{:?}", device.reg4());
        info!("{:?}", device.reg5());

        if let Some(reg_mmio) = device.reg5() {
            info!(
                "{:?}",
                libkernel::memory::falloc::get()
                    .iter()
                    .nth(reg_mmio.physical_addr().frame_index())
            );

            Self {
                device,
                hba_memory: reg_mmio,
            }
        } else {
            panic!("device's host bust adapter is an incorrect register type")
        }
    }

    pub fn hba_memory(&'dev self) -> &'dev HostBustAdapterMemory {
        unsafe { self.hba_memory.read(0).unwrap() }
    }
}
