mod standard;

use crate::memory::mmio::{Mapped, MMIO};
use alloc::vec::Vec;
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
    registers: Vec<Option<MMIO<Mapped>>>,
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
            registers: Vec::new(),
            phantom: PhantomData,
        }),
        0x2 => PCIeDeviceVariant::PCI2CardBus(PCIeDevice::<PCI2CardBus> {
            mmio,
            registers: Vec::new(),
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

    pub fn generic_debut_fmt(&self, debug_struct: &mut fmt::DebugStruct) {
        debug_struct
            .field("Vendor ID", &self.vendor_id())
            .field("Device ID", &self.device_id())
            .field("Command", &self.command())
            .field("Status", &self.status())
            .field("Revision ID", &self.revision_id())
            .field("Class Code", &self.class())
            .field("Sub Class", &self.subclass())
            .field("Cache Line Size", &self.cache_line_size())
            .field("Master Latency Timer", &self.latency_timer())
            .field("Header Type", &self.header_type())
            .field("Built-In Self Test", &self.builtin_self_test());
    }
}

#[derive(Debug)]
pub enum PCIeDeviceRegister {
    MemorySpace32(u32, usize),
    MemorySpace64(u64, usize),
    IOSpace(u32, usize),
    None,
}

impl PCIeDeviceRegister {
    pub fn is_unused(&self) -> bool {
        use bit_field::BitField;

        match self {
            PCIeDeviceRegister::MemorySpace32(value, _) => (value.get_bits(4..32) & !0xFFF) == 0,
            PCIeDeviceRegister::MemorySpace64(value, _) => (value.get_bits(4..64) & !0xFFF) == 0,
            PCIeDeviceRegister::IOSpace(value, _) => (value.get_bits(2..32) & !0xFFF) == 0,
            PCIeDeviceRegister::None => true,
        }
    }

    pub fn as_addr(&self) -> crate::Address<crate::addr_ty::Virtual> {
        use crate::{addr_ty::Virtual, Address};

        Address::<Virtual>::new(match self {
            PCIeDeviceRegister::MemorySpace32(value, _) => (value & !0b1111) as usize,
            PCIeDeviceRegister::MemorySpace64(value, _) => (value & !0b1111) as usize,
            PCIeDeviceRegister::IOSpace(value, _) => (value & !0b11) as usize,
            PCIeDeviceRegister::None => 0,
        })
    }

    pub fn memory_usage(&self) -> usize {
        match self {
            PCIeDeviceRegister::MemorySpace32(_, mem_usage) => *mem_usage,
            PCIeDeviceRegister::MemorySpace64(_, mem_usage) => *mem_usage,
            PCIeDeviceRegister::IOSpace(_, mem_usage) => *mem_usage,
            PCIeDeviceRegister::None => 0,
        }
    }
}

pub struct PCIeDeviceRegisterIterator {
    base: *mut u32,
    max_base: *mut u32,
}

impl PCIeDeviceRegisterIterator {
    unsafe fn new(base: *mut u32, register_count: usize) -> Self {
        Self {
            base,
            max_base: base.add(register_count),
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
                    use bit_field::BitField;

                    if register_raw == 0 {
                        PCIeDeviceRegister::None
                    } else if register_raw.get_bit(0) {
                        self.base.write_volatile(u32::MAX);
                        let mem_usage = (self.base.read_volatile() & !0b11);
                        self.base.write_volatile(register_raw);

                        PCIeDeviceRegister::IOSpace(register_raw, 1)
                    } else {
                        match register_raw.get_bits(1..3) {
                            // REMARK:
                            //  This little dance with the reads & writes is just fucking magic?
                            //  Who comes up with this shit?
                            0b00 => {
                                // Write all 1's to register.
                                self.base.write_volatile(u32::MAX);
                                // Record memory usage by masking address bits, NOT'ing, and adding one.
                                let m1 = self.base.read_volatile() & !0b1111;
                                let mem_usage = !(m1) + 1;
                                info!("{:b} {:b}", m1, mem_usage);
                                // Write original value back into register.
                                self.base.write_volatile(register_raw);

                                PCIeDeviceRegister::MemorySpace32(register_raw, mem_usage as usize)
                            }
                            // And because of MMIO volatility, it's even dumber for 64-bit registers
                            0b10 => {
                                let base_next = self.base.add(1);
                                // Record value of next register to restore later.
                                let register_raw_next = base_next.read_volatile();

                                // Write all 1's into double-wide register.
                                self.base.write(u32::MAX);
                                base_next.write(u32::MAX);

                                // Record raw values of double-wide register.
                                let register_raw_u64 = (self.base.read_volatile() as u64)
                                    | ((base_next.read_volatile() as u64) << 32);

                                // Record memory usage of double-wide register.
                                let mem_usage = !(register_raw_u64 & !0b1111) + 1;

                                // Write old raw values back into double-wide register.
                                self.base.write_volatile(register_raw);
                                base_next.write_volatile(register_raw_next);

                                PCIeDeviceRegister::MemorySpace64(
                                    (register_raw as u64) | ((register_raw_next as u64) << 32),
                                    mem_usage as usize,
                                )
                            }
                            _ => panic!("invalid register type: 0b{:b}", register_raw),
                        }
                    }
                };

                if let PCIeDeviceRegister::MemorySpace64(_, _) = register {
                    self.base = self.base.add(2);
                } else {
                    self.base = self.base.add(1);
                }

                info!("{:?}", register);

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
