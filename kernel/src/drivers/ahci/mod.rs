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

    pub fn configure(&mut self) {
        self.stop_cmd();

        let cmd_base: *mut u8 = libkernel::alloc!(4096, 128);
        let fis_base: *mut u8 = libkernel::alloc!(1024, 128);

        unsafe {
            core::ptr::write_bytes(cmd_base, 0, 4096);
            core::ptr::write_bytes(fis_base, 0, 1024);

            use libkernel::{addr_ty::Virtual, Address};
            self.hba_port
                .set_command_list_base(Address::<Virtual>::from_ptr(cmd_base));
            self.hba_port
                .set_fis_base(Address::<Virtual>::from_ptr(fis_base));
        }

        self.start_cmd();
    }

    pub fn start_cmd(&mut self) {
        let cmd = self.hba_port.command_status();

        while cmd.cr().get() {}

        cmd.fre().set(true);
        cmd.st().set(true);
    }

    pub fn stop_cmd(&mut self) {
        let cmd = self.hba_port.command_status();

        cmd.st().set(false);
        cmd.fre().set(false);

        while cmd.fr().get() | cmd.cr().get() {}
    }
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

                        // This is very unsafe, but it elides the borrow checker, thus allowing us to point to MMIO that's
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
