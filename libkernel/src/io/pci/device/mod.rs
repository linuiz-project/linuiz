mod standard;

use crate::memory::mmio::{Mapped, MMIO};
use bitflags::bitflags;
use core::{fmt, marker::PhantomData};

pub use standard::*;

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum PCIHeaderOffset {
    VendorID = 0x0,
    DeviceID = 0x2,
    Command = 0x4,
    Status = 0x6,
    RevisionID = 0x8,
    ProgramInterface = 0x9,
    Subclass = 0xA,
    Class = 0xB,
    CacheLineSize = 0xC,
    LatencyTimer = 0xD,
    HeaderType = 0xE,
    BuiltInSelfTest = 0xF,
}

impl Into<u32> for PCIHeaderOffset {
    fn into(self) -> u32 {
        self as u32
    }
}

impl Into<usize> for PCIHeaderOffset {
    fn into(self) -> usize {
        self as usize
    }
}

bitflags! {
    pub struct PCIeCommandRegister: u16 {
        // * Read-Only
        const IO_SPACE = 1 << 0;
        // * Read-Only
        const MEMORY_SPACE = 1 << 1;
        // * Read-Write
        const BUS_MASTER = 1 << 2;
        // * Read-Only
        const SPECIAL_CYCLE = 1 << 3;
        // * Read-Only
        const MEMORY_W_AND_I = 1 << 4;
        // * Read-Only
        const VGA_PALETTE_SNOOP = 1 << 5;
        // * Read-Write
        const PARITY_ERROR_RESPONSE = 1 << 6;
        // * Read-Only
        const IDSEL = 1 << 7;
        // * Read-Write
        const SERR_NUM = 1 << 8;
        // * Read-Only
        const FAST_B2B_TRANSACTIONS = 1 << 9;
        // * Read-Write
        const INTERRUPT_DISABLE = 1 << 10;

    }
}

bitflags! {
    pub struct PCIeStatusRegister: u16 {
        const INTERRUPT_STATUS = 1 << 3;
        const CAPABILITIES = 1 << 4;
        // * Not applicable to PCIe.
        const CAPABILITITY_66MHZ = 1 << 5;
        // * Not applicable to PCIe.
        const FAST_BACK2BACK_CAPABLE = 1 << 7;
        const MASTER_DATA_PARITY_ERROR = 1 << 8;
        // * Not applicable to PCIe.
        const DEVSEL_TIMING = 3 << 9;
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

pub trait PCIeDeviceType {
    const REGISTER_COUNT: usize;
}

pub enum Standard {}
impl PCIeDeviceType for Standard {
    const REGISTER_COUNT: usize = 5;
}

pub enum PCI2PCI {}
impl PCIeDeviceType for PCI2PCI {
    const REGISTER_COUNT: usize = 2;
}

pub enum PCI2CardBus {}
impl PCIeDeviceType for PCI2CardBus {
    const REGISTER_COUNT: usize = 8;
}

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
    let type_malfunc = unsafe {
        mmio.read::<u8>(PCIHeaderOffset::HeaderType.into())
            .unwrap()
            .read()
    };

    // mask off the multifunction bit
    match type_malfunc & !(1 << 7) {
        0x0 => PCIeDeviceVariant::Standard(unsafe { PCIeDevice::<Standard>::new(mmio) }),
        0x1 => PCIeDeviceVariant::PCI2PCI(PCIeDevice {
            mmio,
            phantom: PhantomData,
        }),
        0x2 => PCIeDeviceVariant::PCI2CardBus(PCIeDevice::<PCI2CardBus> {
            mmio,
            phantom: PhantomData,
        }),
        invalid_type => {
            panic!(
                "header type is invalid (must be 0..=2): {}, mmio addr: {:?}",
                invalid_type,
                mmio.physical_addr()
            )
        }
    }
}

impl<T: PCIeDeviceType> PCIeDevice<T> {
    pub fn vendor_id(&self) -> u16 {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::VendorID.into())
                .unwrap()
                .read()
        }
    }

    pub fn device_id(&self) -> u16 {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::DeviceID.into())
                .unwrap()
                .read()
        }
    }

    pub fn command(&self) -> PCIeCommandRegister {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::Command.into())
                .unwrap()
                .read()
        }
    }

    pub fn status(&self) -> u16 {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::Status.into())
                .unwrap()
                .read()
        }
    }

    pub fn revision_id(&self) -> u8 {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::RevisionID.into())
                .unwrap()
                .read()
        }
    }

    pub fn program_interface(&self) -> u8 {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::ProgramInterface.into())
                .unwrap()
                .read()
        }
    }

    pub fn subclass(&self) -> u8 {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::Subclass.into())
                .unwrap()
                .read()
        }
    }

    pub fn class(&self) -> PCIeDeviceClass {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::Class.into())
                .unwrap()
                .read()
        }
    }

    pub fn cache_line_size(&self) -> u8 {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::CacheLineSize.into())
                .unwrap()
                .read()
        }
    }

    pub fn latency_timer(&self) -> u8 {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::LatencyTimer.into())
                .unwrap()
                .read()
        }
    }

    pub fn header_type(&self) -> u8 {
        unsafe {
            (self
                .mmio
                .read::<u8>(PCIHeaderOffset::HeaderType.into())
                .unwrap()
                .read())
                & !(1 << 7)
        }
    }

    pub fn multi_function(&self) -> bool {
        unsafe {
            (self
                .mmio
                .read::<u8>(PCIHeaderOffset::BuiltInSelfTest.into())
                .unwrap()
                .read()
                & (1 << 7))
                > 0
        }
    }

    pub fn builtin_self_test(&self) -> PCIeBuiltinSelfTest {
        unsafe {
            self.mmio
                .read(PCIHeaderOffset::BuiltInSelfTest.into())
                .unwrap()
                .read()
        }
    }

    pub fn iter_registers(&self) -> PCIeDeviceRegisterIterator {
        PCIeDeviceRegisterIterator::new(
            unsafe { self.mmio.mapped_addr().as_ptr::<u32>().add(0x4) },
            T::REGISTER_COUNT,
        )
    }

    pub fn generic_debut_fmt(&self, debug_struct: &mut fmt::DebugStruct) {
        debug_struct
            .field("Vendor ID", &self.vendor_id())
            .field("Device ID", &self.device_id())
            .field("Command", &self.command())
            .field("Status", &self.status())
            .field("Revision ID", &self.revision_id())
            .field("Class Code", &self.class())
            .field("Cache Line Size", &self.cache_line_size())
            .field("Master Latency Timer", &self.latency_timer())
            .field("Header Type", &self.header_type())
            .field("Built-In Self Test", &self.builtin_self_test());
    }
}

#[derive(Debug)]
pub enum PCIeDeviceRegister {
    MemorySpace32(u32),
    MemorySpace64(u64),
    IOSpace(u32),
    None,
}

impl PCIeDeviceRegister {
    pub fn as_addr(&self) -> crate::Address<crate::addr_ty::Virtual> {
        use crate::{addr_ty::Virtual, Address};

        Address::<Virtual>::new(match self {
            PCIeDeviceRegister::MemorySpace32(value) => (value & !0b1111) as usize,
            PCIeDeviceRegister::MemorySpace64(value) => (value & !0b1111) as usize,
            PCIeDeviceRegister::IOSpace(value) => (value & !0b11) as usize,
            PCIeDeviceRegister::None => 0,
        })
    }
}

pub struct PCIeDeviceRegisterIterator {
    base: *const u32,
    max_base: *const u32,
}

impl PCIeDeviceRegisterIterator {
    fn new(base: *const u32, register_count: usize) -> Self {
        Self {
            base,
            max_base: unsafe { base.add(register_count) },
        }
    }
}

impl Iterator for PCIeDeviceRegisterIterator {
    type Item = PCIeDeviceRegister;

    fn next(&mut self) -> Option<Self::Item> {
        if self.base <= self.max_base {
            unsafe {
                let register_raw = self.base.read_volatile();

                let register = {
                    if register_raw == 0 {
                        PCIeDeviceRegister::None
                    } else if (register_raw & 0b1) == 0 {
                        use bit_field::BitField;

                        match register_raw.get_bits(1..3) {
                            0b00 => PCIeDeviceRegister::MemorySpace32(register_raw as u32),
                            0b10 => {
                                let lower = self.base.read_volatile();
                                let upper = self.base.add(1).read_volatile();

                                debug!("Creating PCIeDeviceRegister::MemorySpace64:\n\tUpper: {:b}\n\tLower: {:b}", upper, lower );
                                PCIeDeviceRegister::MemorySpace64(
                                    ((upper as u64) << 32) | (lower as u64),
                                )
                            }
                            _ => panic!("invalid register type: 0b{:b}", register_raw),
                        }
                    } else {
                        PCIeDeviceRegister::IOSpace(register_raw as u32)
                    }
                };

                if let PCIeDeviceRegister::MemorySpace64(_) = register {
                    self.base = self.base.add(2);
                } else {
                    self.base = self.base.add(1);
                }

                Some(register)
            }
        } else {
            None
        }
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
