mod standard;

pub use standard::*;

use crate::memory::mmio::{Mapped, MMIO};
use bitflags::bitflags;
use core::{fmt, marker::PhantomData};

bitflags! {
    pub struct PCIeCommandRegister: u16 {
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
    pub struct PCIeStatusRegister: u16 {
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
    pub struct PCIeDeviceSELTiming: u16 {
        const FAST = 0;
        const MEDIUM = 1;
        const SLOW = 1 << 1;
    }
}

impl PCIeStatusRegister {
    pub fn devsel_timing(&self) -> PCIeDeviceSELTiming {
        PCIeDeviceSELTiming::from_bits_truncate((self.bits() >> 9) & 0b11)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PCIeDeviceClass {
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
pub struct PCIeBuiltinSelfTest {
    data: u8,
}

impl PCIeBuiltinSelfTest {
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

impl fmt::Debug for PCIeBuiltinSelfTest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BIST")
            .field("Capable", &self.capable())
            .field("Start", &self.start())
            .field("Completion Code", &self.completion_code())
            .finish()
    }
}

pub trait PCIeDeviceType {}

pub enum Standard {}
impl PCIeDeviceType for Standard {}

pub enum PCI2PCI {}
impl PCIeDeviceType for PCI2PCI {}

pub enum PCI2CardBus {}
impl PCIeDeviceType for PCI2CardBus {}

#[derive(Debug)]
pub enum PCIeDeviceVariant {
    Standard(PCIeDevice<Standard>),
    PCI2PCI(PCIeDevice<PCI2PCI>),
    PCI2CardBus(PCIeDevice<PCI2CardBus>),
}

pub struct PCIeDevice<T: PCIeDeviceType> {
    mmio: MMIO<Mapped>,
    phantom: PhantomData<T>,
}

pub fn new_device(mmio: MMIO<Mapped>) -> PCIeDeviceVariant {
    match unsafe { *mmio.read::<u8>(0xD).unwrap() & 0x7F } {
        0x0 => PCIeDeviceVariant::Standard(PCIeDevice::<Standard> {
            mmio,
            phantom: PhantomData,
        }),
        0x1 => PCIeDeviceVariant::PCI2PCI(PCIeDevice::<PCI2PCI> {
            mmio,
            phantom: PhantomData,
        }),
        0x2 => PCIeDeviceVariant::PCI2CardBus(PCIeDevice::<PCI2CardBus> {
            mmio,
            phantom: PhantomData,
        }),
        invalid_type => panic!("header type is invalid (must be 0..=2): {}", invalid_type),
    }
}

impl<T: PCIeDeviceType> PCIeDevice<T> {
    pub fn vendor_id(&self) -> u16 {
        unsafe { *self.mmio.read(0x0).unwrap() }
    }

    pub fn device_id(&self) -> u16 {
        unsafe { *self.mmio.read(0x2).unwrap() }
    }

    pub fn command(&self) -> PCIeCommandRegister {
        unsafe { *self.mmio.read(0x4).unwrap() }
    }

    pub fn status(&self) -> u16 {
        unsafe { *self.mmio.read(0x6).unwrap() }
    }

    pub fn revision_id(&self) -> u8 {
        unsafe { *self.mmio.read(0x8).unwrap() }
    }

    pub fn program_interface(&self) -> u8 {
        unsafe { *self.mmio.read(0x9).unwrap() }
    }

    pub fn subclass(&self) -> u8 {
        unsafe { *self.mmio.read(0xA).unwrap() }
    }

    pub fn class(&self) -> PCIeDeviceClass {
        unsafe { *self.mmio.read(0xB).unwrap() }
    }

    pub fn cache_line_size(&self) -> u8 {
        unsafe { *self.mmio.read(0xC).unwrap() }
    }

    pub fn latency_timer(&self) -> u8 {
        unsafe { *self.mmio.read(0xD).unwrap() }
    }

    pub fn multi_function(&self) -> bool {
        unsafe { (*self.mmio.read::<u8>(0xE).unwrap() & (1 << 7)) > 0 }
    }

    pub fn builtin_self_test(&self) -> PCIeBuiltinSelfTest {
        unsafe { *self.mmio.read(0xE).unwrap() }
    }
}

impl fmt::Debug for PCIeDevice<PCI2PCI> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ss").finish()
    }
}

impl fmt::Debug for PCIeDevice<PCI2CardBus> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ss").finish()
    }
}
