mod command_status;

use core::convert::TryFrom;
use libkernel::{addr_ty::Virtual, Address};
use num_enum::TryFromPrimitive;

pub use command_status::*;

#[repr(C)]
pub struct HBAPRDTEntry {
    db_addr_lower: u32,
    db_addr_upper: u32,
    rsvd0: u32,
    bits1: u32,
}

impl HBAPRDTEntry {
    libkernel::bitfield_getter!(bits1, u32, byte_count, 0..22);
    libkernel::bitfield_getter!(bits1, interrupt_on_completion, 31);

    pub fn set_db_addr(&mut self, addr: libkernel::Address<libkernel::addr_ty::Virtual>) {
        let addr_usize = addr.as_usize();

        self.db_addr_lower = addr_usize as u32;
        self.db_addr_upper = (addr_usize >> 32) as u32;
    }

    pub fn set_sector_count(&mut self, sector_count: u32) {
        self.set_byte_count(
            (sector_count << 9) - 1, /* 512-byte alignment per sector */
        );
    }

    pub fn clear(&mut self) {
        self.db_addr_lower = 0;
        self.db_addr_upper = 0;
        self.bits1 = 0;
    }
}

pub trait CommandFIS {}

#[repr(C)]
pub struct HBACommandTable {
    command_fis: [u8; 64],
    atapi_command: [u8; 16],
    rsvd0: [u8; 48],
    prdt: core::ffi::c_void,
}

impl HBACommandTable {
    pub fn prdt_entries(&mut self, entry_count: u16) -> &mut [HBAPRDTEntry] {
        unsafe {
            core::slice::from_raw_parts_mut(
                (&mut self.prdt) as *mut _ as *mut HBAPRDTEntry,
                entry_count as usize,
            )
        }
    }

    pub fn clear(&mut self, prdt_entry_count: u16) {
        self.command_fis.fill(0);
        self.atapi_command.fill(0);
        self.prdt_entries(prdt_entry_count)
            .iter_mut()
            .for_each(|entry| entry.clear());
    }

    pub fn command_fis<T: CommandFIS>(&mut self) -> &mut T {
        unsafe { &mut *(self.command_fis.as_mut_ptr() as *mut _) }
    }
}

#[repr(C)]
pub struct HBACommandHeader {
    bits1: u16,
    prdt_len: u16,
    prdb_count: u32,
    cmd_tbl_addr_lower: u32,
    cmd_tbl_addr_upper: u32,
    reserved1: [u8; 4],
}

impl HBACommandHeader {
    libkernel::bitfield_getter!(bits1, u16, fis_len, 0..5);
    libkernel::bitfield_getter!(bits1, atapi, 5);
    libkernel::bitfield_getter!(bits1, write, 6);
    libkernel::bitfield_getter!(bits1, prefetchable, 7);
    libkernel::bitfield_getter!(bits1, reset, 8);
    libkernel::bitfield_getter!(bits1, bist, 9);
    libkernel::bitfield_getter!(bits1, clear_busy_on_rok, 10);
    libkernel::bitfield_getter!(bits1, u16, port_multiplier, 12..16);

    pub fn prdt_len(&mut self) -> &mut u16 {
        &mut self.prdt_len
    }

    pub unsafe fn set_command_table_base_addr(&mut self, addr: Address<Virtual>) {
        let addr_usize = addr.as_usize();

        self.cmd_tbl_addr_lower = addr_usize as u32;
        self.cmd_tbl_addr_upper = (addr_usize >> 32) as u32;
    }

    pub fn command_tables(&mut self) -> Option<&mut [HBACommandTable]> {
        if self.cmd_tbl_addr_lower > 0 || self.cmd_tbl_addr_upper > 0 {
            Some(unsafe {
                core::slice::from_raw_parts_mut(
                    ((self.cmd_tbl_addr_lower as usize)
                        | ((self.cmd_tbl_addr_upper as usize) << 32)) as *mut _,
                    32,
                )
            })
        } else {
            None
        }
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

#[repr(u32)]
#[derive(TryFromPrimitive)]
pub enum DeviceDetectionInitialization {
    None = 0,
    FullReinit = 1,
    DisbaleSATA = 4,
}

#[repr(u32)]
#[derive(TryFromPrimitive)]
pub enum SpeedAllowed {
    NoRestriction = 0,
    Gen1 = 1,
    Gen2 = 2,
    Gen3 = 3,
}

// IPWM = Interface Power Management
#[repr(u32)]
#[derive(TryFromPrimitive)]
pub enum IPWMTransitionsAllowed {
    NoRestriction = 0,
    PartialStateDisabled = 1,
    SlumberStateDisabled = 2,
    PartialAndSlumberStateDisabled = 3,
    DevSleepPWMStateDisabled = 4,
    PartialAndDevSleepPWNDisabled = 5,
    SlumberAndDevSleepPWMDisabled = 6,
    AllDisabled = 7,
}

#[repr(C)]
#[derive(Debug)]
pub struct HBAPort {
    cmd_list_addr_lower: u32,
    cmd_list_addr_upper: u32,
    fis_addr_lower: u32,
    fis_addr_upper: u32,
    interrupt_status: u32,
    interrupt_enable: u32,
    command_status: CommandStatus,
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

impl HBAPort {
    pub fn signature(&self) -> Result<HostBusAdapterPortClass, u32> {
        match self.signature {
            0x00000101 => Ok(HostBusAdapterPortClass::SATA),
            0xC33C0101 => Ok(HostBusAdapterPortClass::SEMB),
            0x96690101 => Ok(HostBusAdapterPortClass::PM),
            0xEB140101 => Ok(HostBusAdapterPortClass::SATAPI),
            signature => Err(signature),
        }
    }

    pub unsafe fn set_command_list_addr(&mut self, addr: Address<Virtual>) {
        let addr_usize = addr.as_usize();

        self.cmd_list_addr_lower = addr_usize as u32;
        self.cmd_list_addr_upper = (addr_usize >> 32) as u32;
    }

    pub unsafe fn set_fis_addr(&mut self, addr: Address<Virtual>) {
        let addr_usize = addr.as_usize();

        self.fis_addr_lower = addr_usize as u32;
        self.fis_addr_upper = (addr_usize >> 32) as u32;
    }

    pub fn sata_status(&self) -> &SATAStatus {
        &self.sata_status
    }

    pub fn command_status(&mut self) -> &mut CommandStatus {
        &mut self.command_status
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
            self.signature().expect("invalid port signature")
        }
    }

    pub fn task_file_data(&self) -> u32 {
        self.task_file_data
    }

    pub fn command_list(&mut self) -> Option<&mut [HBACommandHeader]> {
        if self.cmd_list_addr_lower > 0 || self.cmd_list_addr_upper > 0 {
            Some(unsafe {
                core::slice::from_raw_parts_mut(
                    ((self.cmd_list_addr_lower as usize)
                        | ((self.cmd_list_addr_upper as usize) << 32))
                        as *mut _,
                    32,
                )
            })
        } else {
            None
        }
    }

    pub fn interrupt_status(&mut self) -> &mut u32 {
        // todo make sense of this register
        &mut self.interrupt_status
    }

    pub fn command_issue(&mut self) -> &mut u32 {
        &mut self.command_issue
    }
}
