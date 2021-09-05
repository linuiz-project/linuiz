pub mod hba;

use alloc::vec::Vec;
use bit_field::BitField;
use libkernel::io::pci::{PCIeDevice, Standard, StandardRegister};

pub const ATA_DEV_BUSY: u8 = 0x80;
pub const ATA_DEV_DRQ: u8 = 0x08;
pub const ATA_CMD_READ_DMA_EX: u8 = 0x25;
pub const HBA_PxIS_TFES: u32 = 1 << 30;

#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum FISType {
    None = 0x0,
    H2D = 0x27,
    D2H = 0x34,
    DMA_ACT = 0x39,
    DMA_SETUP = 0x41,
    DATA = 0x46,
    BIST = 0x48,
    PIO_SETUP = 0x5F,
    DEV_BITS = 0xA1,
}

#[repr(C)]
pub struct FIS_REG_H2D {
    fis_type: FISType,
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
    count_low: u8,
    count_high: u8,
    iso_cmd_completion: u8,
    control: u8,
    rsvd0: [u8; 4],
}

impl FIS_REG_H2D {
    libkernel::bitfield_getter!(bits1, u8, port_multiplier, 0..4);
    libkernel::bitfield_getter!(bits1, command_control, 7);

    pub fn set_sector_base(&mut self, sector: usize) {
        assert_eq!(sector & 0xFFFFFFFFFFFF, 0, "sector is 48 bits");

        self.lba0 = sector.get_bits(0..8) as u8;
        self.lba1 = sector.get_bits(8..16) as u8;
        self.lba2 = sector.get_bits(16..24) as u8;
        self.lba3 = sector.get_bits(24..32) as u8;
        self.lba4 = sector.get_bits(32..40) as u8;
        self.lba5 = sector.get_bits(40..48) as u8;
    }

    pub fn set_sector_count(&mut self, sectors: u16) {
        self.count_low = sectors as u8;
        self.count_high = (sectors >> 8) as u8;
    }
}

impl Default for FIS_REG_H2D {
    fn default() -> Self {
        Self {
            fis_type: FISType::None,
            bits1: 0,
            command: 0,
            feature_low: 0,
            lba0: 0,
            lba1: 0,
            lba2: 0,
            device_register: 0,
            lba3: 0,
            lba4: 0,
            lba5: 0,
            feature_high: 0,
            count_low: 0,
            count_high: 0,
            iso_cmd_completion: 0,
            control: 0,
            rsvd0: [0u8; 4],
        }
    }
}

impl hba::HBACommandFIS for FIS_REG_H2D {}

pub struct AHCI<'ahci> {
    device: &'ahci PCIeDevice<Standard>,
    sata_ports: Vec<&'ahci mut self::hba::HBAPort>,
}

impl<'ahci> AHCI<'ahci> {
    pub fn from_pcie_device(device: &'ahci PCIeDevice<Standard>) -> Self {
        trace!("Using PCIe device for AHCI driver:\n{:#?}", device);

        if let Some(mut hba_register) = device.get_register(StandardRegister::Reg5) {
            use hba::HBAPortClass;

            let sata_ports = unsafe { hba_register.read_mut::<hba::HBAMemory>(0) }
                .unwrap()
                .borrow_mut()
                .ports_mut()
                .filter_map(|port| match port.class() {
                    HBAPortClass::SATA | HBAPortClass::SATAPI => {
                        debug!("Configuring AHCI port: {:?}", port.class());

                        Some(unsafe {
                            // Elide borrow checker by casting to a pointer & back.
                            &mut *(port as *mut _)
                        })
                    }
                    _port_type => None,
                })
                .collect();

            Self { device, sata_ports }
        } else {
            panic!("device's host bust adapter is an incorrect register type")
        }
    }

    pub fn sata_ports(&'ahci mut self) -> core::slice::IterMut<&'ahci mut self::hba::HBAPort> {
        self.sata_ports.iter_mut()
    }
}
