#![allow(dead_code)]

pub mod express;
pub mod legacy;

use bitflags::bitflags;
use core::fmt;

bitflags! {
    pub struct PCICommandRegister: u16 {
        const IO_SPACE = 1 << 0;
        const MEMORY_SPACE = 1 << 1;
        const BUS_MASTER = 1 << 2;
        const SPECIAL_CYCLES = 1 << 3;
        const MEMORY_W_AND_I_ENABLE = 1 << 4;
        const VGA_PALETTE_SNOOP = 1 << 5;
        const PARITY_ERROR_RESPONSE = 1 << 6;
        const SERR_ENABLE = 1 << 8;
        const FAST_B2B_ENABLE = 1 << 9;
        const INTERRUPT_DISABLE = 1 << 10;
    }
}

bitflags! {
    pub struct PCIStatusRegister: u16 {
        const CAPABILITIES = 1 << 4;
        const CAPABILITITY_66MHZ = 1 << 5;
        const FAST_BACK2BACK_CAPABLE = 1 << 7;
        const MASTER_DATA_PARITY_ERROR = 1 << 8;
        const SIGNALED_TARGET_ABORT = 1 << 11;
        const RECEIVED_TARGET_ABORT = 1 << 12;
        const RECEIVED_MASTER_ABORT =  1 << 13;
        const SIGNALED_SYSTEM_ERROR = 1 << 14;
        const DETECTED_PARITY_ERROR = 1 << 15;
    }
}

bitflags! {
    pub struct PCIDeviceSELTiming: u16 {
        const FAST = 0;
        const MEDIUM = 1;
        const SLOW = 1 << 1;
    }
}

impl PCIStatusRegister {
    pub fn devsel_timing(&self) -> PCIDeviceSELTiming {
        PCIDeviceSELTiming::from_bits_truncate((self.bits() >> 9) & 0b11)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PCIDeviceClass {
    Unclassified = 0x0,
    MassStorageController = 0x1,
    NetworkController = 0x2,
    DisplayController = 0x3,
    MultimediaController = 0x4,
    MemoryController = 0x5,
    Bridge = 0x6,
    CommunicationController = 0x7,
    GenericSystemPeripheral = 0x8,
    InputDeviceController = 0x9,
    DockingStation = 0xA,
    Processor = 0xB,
    SerialBusController = 0xC,
    WirelessController = 0xD,
    IntelligentController = 0xE,
    SatelliteCommunicationsController = 0xF,
    EncryptionController = 0x10,
    SignalProcessingController = 0x11,
    ProcessingAccelerators = 0x12,
    NonEssentialInstrumentation = 0x13,
    Coprocessor = 0x40,
    Unassigned = 0xFF,
}

#[repr(transparent)]
pub struct PCIBISTRegister {
    data: u8,
}

impl PCIBISTRegister {
    pub fn capable(&self) -> bool {
        (self.data & (1 << 7)) > 0
    }

    pub fn start(&self) -> bool {
        (self.data & (1 << 6)) > 0
    }

    pub fn completion_code(&self) -> u8 {
        self.data & 0b111
    }
}

impl core::fmt::Debug for PCIBISTRegister {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BIST")
            .field("Capable", &self.capable())
            .field("Start", &self.start())
            .field("Completion Code", &self.completion_code())
            .finish()
    }
}

#[repr(C, align(0x100))]
pub struct PCIDeviceHeader {
    vendor_id: u16,
    device_id: u16,
    command: PCICommandRegister,
    status: PCIStatusRegister,
    revision_id: u8,
    prog_interface: u8,
    /// Subclass of the PCI device class.
    ///
    /// An explanation for this value can be found at:
    ///   https://pcisig.com/sites/default/files/files/PCI_Code-ID_r_1_11__v24_Jan_2019.pdf
    subclass: u8,
    class: PCIDeviceClass,
    cache_line_size: u8,
    latency_timer: u8,
    header_type: u8,
    bist: PCIBISTRegister,
}

impl PCIDeviceHeader {
    pub fn is_invalid(&self) -> bool {
        self.vendor_id() == u16::MAX || self.device_id() == u16::MAX
    }

    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    pub fn vendor_str(&self) -> &str {
        match self.vendor_id() {
            0x8086 => &"INTEL",
            0x1022 => &"AMD",
            0x1DDE => &"NVIDIA",
            _ => &"Unknown",
        }
    }

    pub fn device_id(&self) -> u16 {
        self.device_id
    }

    pub fn device_str(&self) -> &str {
        match self.vendor_id() {
            0x8086 if self.device_id() == 0x29C0 => &"82G33/G31/P35/P31 Express DRAM Controller",
            0x8086 if self.device_id() == 0x2922 => {
                &"82801IR/IO/IH (ICH9R/DO/DH) 6 port SATA Controller [AHCI mode]"
            }

            _ => &"Unknown",
        }
    }

    pub fn command(&self) -> PCICommandRegister {
        self.command
    }

    pub fn status(&self) -> PCIStatusRegister {
        self.status
    }

    pub fn revision_id(&self) -> u8 {
        self.revision_id
    }

    pub fn program_interface(&self) -> u8 {
        self.prog_interface
    }

    pub fn subclass(&self) -> u8 {
        self.subclass
    }

    pub fn class(&self) -> PCIDeviceClass {
        self.class
    }

    pub fn cache_line_size(&self) -> u8 {
        self.cache_line_size
    }

    pub fn latency_timer(&self) -> u8 {
        self.latency_timer
    }

    pub fn header_type(&self) -> u8 {
        self.header_type & 0b111111
    }

    pub fn multi_function(&self) -> bool {
        (self.header_type & (1 << 7)) > 0
    }

    pub fn bist(&self) -> &PCIBISTRegister {
        &self.bist
    }
}

impl fmt::Debug for PCIDeviceHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PCIDeviceHeader")
            .field("Vendor", &self.vendor_str())
            .field("Vendor ID", &(self.vendor_id() as *const u8))
            .field("Device", &self.device_str())
            .field("Device ID", &(self.device_id() as *const u8))
            .field("Command", &self.command())
            .field("Status", &self.status())
            .field("Revision ID", &self.revision_id())
            .field("Program Interface", &self.program_interface())
            .field("Subclass", &self.subclass())
            .field("Class", &self.class())
            .field("Cache Line Size", &self.cache_line_size())
            .field("Latency Timer", &self.latency_timer())
            .field("Multi-Function", &self.multi_function())
            .field("Header Type", &self.header_type())
            .field("BIST", self.bist())
            .finish()
    }
}
