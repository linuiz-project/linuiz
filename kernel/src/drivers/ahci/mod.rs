pub mod hba;

use alloc::vec::Vec;
use hba::{HostBusAdapterPort, HostBusAdapterPortClass, HostBustAdapterMemory};
use libkernel::{
    io::pci::{PCIeDevice, Standard},
    memory::mmio::{Mapped, MMIO},
};
use spin::{MutexGuard, RwLock};

#[derive(Debug)]
pub struct AHCIPort<'hba> {
    port_num: u8,
    hba_port: &'hba HostBusAdapterPort,
    buffer: [u8; 2048],
}

impl<'hba> AHCIPort<'hba> {
    fn new(port_num: u8, hba_port: &'hba HostBusAdapterPort) -> Self {
        Self {
            port_num,
            hba_port,
            buffer: [0u8; 2048],
        }
    }
}

pub struct AHCI<'dev, 'port> {
    device: &'dev PCIeDevice<Standard>,
    hba_memory: MutexGuard<'dev, MMIO<Mapped>>,
    ports: RwLock<Vec<AHCIPort<'port>>>,
}

impl<'dev, 'port> AHCI<'dev, 'port> {
    pub fn from_pcie_device(device: &'dev PCIeDevice<Standard>) -> Self {
        trace!("Using PCIe device for AHCI driver:\n{:#?}", device);

        if let Some(reg_mmio) = device.reg5() {
            Self {
                device,
                hba_memory: reg_mmio,
                ports: RwLock::new(Vec::new()),
            }
        } else {
            panic!("device's host bust adapter is an incorrect register type")
        }
    }

    pub fn hba_memory(&'dev self) -> &'dev HostBustAdapterMemory {
        unsafe { self.hba_memory.read(0).unwrap() }
    }

    pub fn configure_ports(&'port self) {
        let mut ports = self.ports.write();

        if ports.len() > 0 {
            warn!("AHCI driver is re-enumerating ports (this should only be done once)!");
        } else {
            ports.clear();
            ports.extend(
                unsafe { self.hba_memory.read::<HostBustAdapterMemory>(0).unwrap() }
                    .ports()
                    .enumerate()
                    .filter_map(|(port_num, port)| match port.class() {
                        HostBusAdapterPortClass::SATA | HostBusAdapterPortClass::SATAPI => {
                            Some(AHCIPort::new(port_num as u8, port))
                        }
                        _port_type => None,
                    }),
            )
        }
    }

    pub fn ports(&self) -> spin::RwLockReadGuard<Vec<AHCIPort<'port>>> {
        self.ports.read()
    }
}
