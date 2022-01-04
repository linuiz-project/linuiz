pub mod hba;

use alloc::vec::Vec;
use bit_field::BitField;
use libstd::io::pci::{standard::StandardRegister, PCIeDevice, Standard};

pub const ATA_DEV_BUSY: u8 = 0x80;
pub const ATA_DEV_DRQ: u8 = 0x08;

#[repr(u8)]
pub enum CommandType {
    ReadDMA = 0x25,
}

#[allow(non_upper_case_globals)]
pub const HBA_PxIS_TFES: u32 = 1 << 30;

pub struct AHCI<'ahci> {
    // TODO devise some system of renting out the PCIe devices to drivers
    device: &'ahci PCIeDevice<Standard>,
    sata_ports: Vec<&'ahci hba::Port>,
}

impl<'ahci> AHCI<'ahci> {
    pub fn from_pcie_device(device: &'ahci PCIeDevice<Standard>) -> Self {
        assert_eq!(
            device.class(),
            libstd::io::pci::DeviceClass::MassStorageController,
            "Device provided for AHCI driver must be MassStorageController."
        );
        assert_eq!(
            device.subclass(),
            0x6,
            "Device provided for AHCI driver must be subclass Serial ATA."
        );

        debug!("Using PCIe device for AHCI driver:\n{:#?}", device);

        if let Some(hba_mmio) = device.get_register(StandardRegister::Register5) {
            debug!("Parsing valid SATA ports from HBA memory ports.");
            let sata_ports = unsafe { hba_mmio.borrow::<hba::Memory>(0x0) }
                .ports()
                .filter_map(|port| match port.class() {
                    hba::Class::SATA | hba::Class::SATAPI => {
                        debug!("Configuring AHCI port: {:?}", port.class());
                        port.configure();
                        Some(port)
                    }
                    _port_type => None,
                })
                .collect::<Vec<&hba::Port>>();

            debug!("Found SATA ports: {}", sata_ports.len());

            Self { device, sata_ports }
        } else {
            panic!("device's host bust adapter is an incorrect register type")
        }
    }

    pub fn sata_ports(&'ahci self) -> core::slice::Iter<&'ahci hba::Port> {
        self.sata_ports.iter()
    }
}
