use crate::{addr_ty::Virtual, io::pci::express::PCIeDevice, Address};
use alloc::vec::Vec;

pub struct PCIeBus {
    base_addr: Address<Virtual>,
    devices: Vec<PCIeDevice>,
}

impl PCIeBus {
    // const unsafe fn empty() -> Self {
    //     Self {
    //         devices:
    //     }
    // }
}
