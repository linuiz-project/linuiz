use core::convert::TryFrom;

use libkernel::{
    io::pci::{PCIeDevice, Standard},
    memory::mmio::{Mapped, MMIO},
};
use num_enum::TryFromPrimitive;
use spin::MutexGuard;

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum InterfacePowerManagement {
    NonCommunicate = 0,
    Active = 1,
    Partial = 2,
    Slumber = 6,
    DevSleep = 8,
}

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum InterfaceSpeed {
    NonCommunicate = 0,
    Gen1 = 1,
    Gen2 = 2,
    Gen3 = 3,
}

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum DeviceDetection {
    NonCommunicate = 0,
    DetectedNoPhy = 1,
    DetectedAndPhy = 3,
    PhyOffline = 4,
}

#[repr(transparent)]
pub struct SATAStatus {
    status: u32,
}

impl SATAStatus {
    pub fn interface_pwm(&self) -> InterfacePowerManagement {
        InterfacePowerManagement::try_from((self.status >> 8) & 0b1111).unwrap()
    }

    pub fn interface_speed(&self) -> InterfaceSpeed {
        InterfaceSpeed::try_from((self.status >> 4) & 0b1111).unwrap()
    }

    pub fn device_detection(&self) -> DeviceDetection {
        DeviceDetection::try_from(self.status & 0b1111).unwrap()
    }
}

impl core::fmt::Debug for SATAStatus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SATA Port Status")
            .field("Interface PWM", &self.interface_pwm())
            .field("Interface Speed", &self.interface_speed())
            .field("Device Detection", &self.device_detection())
            .finish()
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct HostBusAdapterPort {
    /// Note: In the specificaiton, this is two 32-bit values
    command_list_base: u64,
    fis_base_address: u64,
    interrupt_status: u32,
    interrupt_enable: u32,
    command_status: u32,
    _reserved0: [u8; 0x4],
    task_file_data: u32,
    signature: u32,
    sata_status: SATAStatus,
    sata_control: u32,
    sata_error: u32,
    sata_active: u32,
    command_issue: u32,
    sata_notification: u32,
    fis_switch_control: u32,
    _reserved1: [u8; 0xB],
    _vendor0: [u8; 0x4],
}

impl HostBusAdapterPort {
    pub fn signature(&self) -> u32 {
        self.signature
    }

    pub fn sata_status(&self) -> &SATAStatus {
        &self.sata_status
    }

    pub fn class(&self) -> HostBusAdapterPortClass {
        let sata_status = self.sata_status();
        if !matches!(
            sata_status.device_detection(),
            DeviceDetection::DetectedAndPhy
        ) || !matches!(
            sata_status.interface_pwm(),
            InterfacePowerManagement::Active
        ) {
            HostBusAdapterPortClass::None
        } else {
            match self.signature() {
                // ATAPI
                0xEB140101 => HostBusAdapterPortClass::SATAPI,
                // ATA
                0x00000101 => HostBusAdapterPortClass::SATA,
                // SEMB
                0xC33C0101 => HostBusAdapterPortClass::SEMB,
                // PM
                0x96690101 => HostBusAdapterPortClass::PM,
                // fail state
                signature => panic!("invalid port signature: 0x{:X}", signature),
            }
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct HostBustAdapterMemory {
    host_capability: u32,
    global_host_control: u32,
    interrupt_status: u32,
    ports_implemented: u32,
    version: u32,
    ccc_control: u32,
    ccc_ports: u32,
    enclosure_management_location: u32,
    enclosure_management_control: u32,
    host_capabilities_extended: u32,
    bios_handoff_control_status: u32,
    _reserved0: [u8; 0x74],
    _vendor0: [u8; 0x60],
    ports: [HostBusAdapterPort; 32],
}

impl HostBustAdapterMemory {
    // pub const fn ports_implemented(&self) -> u32 {
    //     self.ports_implemented
    // }

    pub const fn ports(&self) -> &[HostBusAdapterPort; 32] {
        &self.ports
    }
}

#[derive(Debug)]
pub enum HostBusAdapterPortClass {
    None,
    SATA,
    SEMB,
    PM,
    SATAPI,
}

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
