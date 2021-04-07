use libkernel::{
    io::pci::PCIeDevice,
    memory::mmio::{Mapped, MMIO},
};

pub struct AHCI {
    mmio: MMIO<Mapped>,
}

// impl AHCI {
//     pub fn from_pcie_device(pcie_device: PCIeDevice) -> Self {
//         trace!("Using PCIe device for AHCI driver:\n{:#?}", pcie_device);

//         Self {
//             mmio: pcie_device.consume_mmio(),
//         }
//     }
// }
