pub mod hba;

use alloc::vec::Vec;
use hba::{
    port::{HostBusAdapterPort, HostBusAdapterPortClass},
    HostBustAdapterMemory,
};
use libkernel::io::pci::{PCIeDevice, Standard, StandardRegister};

#[repr(C)]
pub struct FIS_REG_H2D {
    fis_type: u8,
    bits1: u8,
    command: u8,
    feature_low: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device_register: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    feature_high: u8,
    // NOTE: This is two u8's in the spec
    count: u16,
    iso_cmd_completion: u8,
    control: u8,
    rsvd0: [u8; 4]
}

impl FIS_REG_H2D {
    libkernel::bitfield_getter!(bits1, u8, port_multiplier, 0..4);
    libkernel::bitfield_getter!(bits1, command_control, 7);
}

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

    pub fn start_cmd(&mut self) {
        debug!("AHCI PORT #{}: START_CMD", self.port_num);

        let cmd = self.hba_port.command_status();

        while cmd.cr().get() {}

        cmd.fre().set(true);
        cmd.st().set(true);
    }

    pub fn stop_cmd(&mut self) {
        debug!("AHCI PORT #{}: STOP_CMD", self.port_num);

        let cmd = self.hba_port.command_status();

        cmd.st().set(false);
        cmd.fre().set(false);

        while cmd.fr().get() | cmd.cr().get() {}
    }

    pub fn configure(&mut self) {
        debug!("AHCI PORT #{}: CONFIGURING.", self.port_num);

        self.stop_cmd();

        debug!("Allocting command and FIS lists.");

        let cmd_base: *mut u8 = libkernel::alloc!(4096, 128);
        let fis_base: *mut u8 = libkernel::alloc!(1024, 128);

        unsafe {
            debug!("Clearing command and FIS lists, and updating port metadata.");

            core::ptr::write_bytes(cmd_base, 0, 4096);
            core::ptr::write_bytes(fis_base, 0, 1024);

            use libkernel::{addr_ty::Virtual, Address};
            self.hba_port
                .set_command_list_base(Address::<Virtual>::from_ptr(cmd_base));
            self.hba_port
                .set_fis_base(Address::<Virtual>::from_ptr(fis_base));
        }

        debug!("Configuring individual command headers.");

        for (index, cmd_header) in self
            .hba_port
            .command_list_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            cmd_header.set_prdt_len(8);

            unsafe {
                let cmd_table_addr: *mut u8 = libkernel::alloc!(4096, 128);
                core::ptr::write_bytes(cmd_table_addr, 0, 4096);

                debug!(
                    "Configured command header #{} with table address {:?}",
                    index, cmd_table_addr
                );

                cmd_header.set_command_table_base_addr(libkernel::Address::<
                    libkernel::addr_ty::Virtual,
                >::from_ptr(cmd_table_addr))
            };
        }

        self.start_cmd();

        debug!("AHCI PORT #{}: CONFIGURED.", self.port_num);
    }

    pub fn read(&mut self, sectors: Range<usize>) -> Vec<u8> {
        self.hba_port.clear_interrupts();
        let cmd_headers = self.hba_port.command_list_mut().expect("AHCI port's HBA has not yet been configured");
        let cmd_header = &mut cmd_headers[1];
        cmd_header.set_fis_len(core::mem::size_of::<FIS_REG_H2D>() / u32);
        cmd_header.set_write(false);
        cmd_header.set_prdt_len(1);

        
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
