pub mod hba;

use alloc::vec::Vec;
use hba::{
    port::{HostBusAdapterPort, HostBusAdapterPortClass},
    HostBustAdapterMemory,
};
use libkernel::io::pci::{PCIeDevice, Standard, StandardRegister};

#[derive(Debug)]
pub struct AHCIPort<'hba> {
    port_num: u8,
    hba_port: &'hba mut HostBusAdapterPort,
    buffer: [u8; 2048],
}

impl<'hba> AHCIPort<'hba> {
    fn new(port_num: u8, hba_port: &'hba mut HostBusAdapterPort) -> Self {
        Self {
            port_num,
            hba_port,
            buffer: [0u8; 2048],
        }
    }

    pub fn hba(&mut self) -> &mut HostBusAdapterPort {
        self.hba_port
    }

    pub fn configure() {}

    pub fn start_cmd(&self) {}

    pub fn stop_cmd(&self) {}
}

pub struct AHCI<'ahci> {
    ports: Vec<AHCIPort<'ahci>>,
}

impl<'ahci> AHCI<'ahci> {
    pub fn from_pcie_device(device: &'ahci PCIeDevice<Standard>) -> Self {
        trace!("Using PCIe device for AHCI driver:\n{:#?}", device);

        if let Some(mut hba_register) = device.get_register_locked(StandardRegister::Reg5) {
            // Allows this context to 'own' and move around values derived from HBA memory.
            let own_hba_memory = hba_register
                .mapped_addr()
                .as_mut_ptr::<HostBustAdapterMemory>();

            let ports = unsafe { hba_register.read_mut::<HostBustAdapterMemory>(0).unwrap() }
                .ports()
                .iter()
                .enumerate()
                .filter_map(|(port_num, port)| match port.class() {
                    HostBusAdapterPortClass::SATA | HostBusAdapterPortClass::SATAPI => {
                        debug!("Configuring AHCI port #{}: {:?}", port_num, port.class());

                        // This is very unsafe, but it elides the borrow checker, a la allowing us to point to MMIO that's
                        //  TECHNICALLY owned by the `device`.
                        let own_port = unsafe { &mut ((*own_hba_memory).ports_mut()[port_num]) };
                        Some(AHCIPort::new(port_num as u8, own_port))
                    }
                    _port_type => None,
                })
                .collect();

            Self { ports }
        } else {
            panic!("device's host bust adapter is an incorrect register type")
        }
    }

    pub fn iter(&'ahci self) -> core::slice::Iter<AHCIPort<'ahci>> {
        self.ports.iter()
    }

    pub fn iter_mut(&'ahci mut self) -> core::slice::IterMut<AHCIPort<'ahci>> {
        self.ports.iter_mut()
    }
}
