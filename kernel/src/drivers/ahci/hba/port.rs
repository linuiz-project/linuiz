use core::convert::TryFrom;
use num_enum::TryFromPrimitive;

#[derive(Debug)]
pub enum HostBusAdapterPortClass {
    None,
    SATA,
    SEMB,
    PM,
    SATAPI,
}

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
        // Ensures port is in a valid state (deteced & powered).
        if !matches!(
            self.sata_status().device_detection(),
            DeviceDetection::DetectedAndPhy
        ) || !matches!(
            self.sata_status().interface_pwm(),
            InterfacePowerManagement::Active
        ) {
            HostBusAdapterPortClass::None
        } else {
            // Finally, determine port type from its signature.
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

pub struct HostBusAdapterPortIterator<'hba> {
    ports: &'hba [HostBusAdapterPort; 32],
    ports_implemented: u32,
    cur_index: usize,
}

impl<'hba> HostBusAdapterPortIterator<'hba> {
    pub(super) fn new(ports: &'hba [HostBusAdapterPort; 32], ports_implemented: u32) -> Self {
        Self {
            ports,
            ports_implemented,
            cur_index: 0,
        }
    }
}

impl<'hba> Iterator for HostBusAdapterPortIterator<'hba> {
    type Item = &'hba HostBusAdapterPort;

    fn next(&mut self) -> Option<Self::Item> {
        if (self.ports_implemented & (1 << self.cur_index)) > 0 {
            let port = &self.ports[self.cur_index];
            self.cur_index += 1;

            Some(port)
        } else {
            None
        }
    }
}
