mod bus;
mod device;
mod host;

pub use bus::*;
pub use device::*;
pub use host::*;

bitflags::bitflags! {
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

bitflags::bitflags! {
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

bitflags::bitflags! {
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
#[derive(Clone, Copy)]
pub struct PCIBuiltinSelfTest {
    data: u8,
}

impl PCIBuiltinSelfTest {
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

impl core::fmt::Debug for PCIBuiltinSelfTest {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BIST")
            .field("Capable", &self.capable())
            .field("Start", &self.start())
            .field("Completion Code", &self.completion_code())
            .finish()
    }
}
