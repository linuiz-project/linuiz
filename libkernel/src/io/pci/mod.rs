#![allow(dead_code)]

pub mod express;
pub mod legacy;

use bitflags::bitflags;
use core::fmt;

bitflags! {
    pub struct PCIDeviceStatus: u16 {
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

impl PCIDeviceStatus {
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
    command: u16,
    status: u16,
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
        self.vendor_id() == u16::MAX
    }

    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    pub fn vendor(&self) -> &str {
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

    pub fn status(&self) -> PCIDeviceStatus {
        PCIDeviceStatus::from_bits_truncate(self.status)
    }

    pub fn class(&self) -> PCIDeviceClass {
        self.class
    }

    pub fn subclass(&self) -> u8 {
        self.subclass
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
            .field("Vendor", &self.vendor())
            .field("Device ID", &self.device_id())
            .field("Class", &self.class())
            .field("Subclass", &self.subclass())
            .field("Status", &self.status())
            .field("Multi-Function", &self.multi_function())
            .field("Header Type", &self.header_type())
            .field("BIST", self.bist())
            .finish()
    }
}

pub trait BARLayoutVariant {}

pub enum MemorySpace {}
impl BARLayoutVariant for MemorySpace {}

pub enum IOSpace {}
impl BARLayoutVariant for IOSpace {}

#[derive(Debug)]
pub enum BaseAddressRegisterType<'a> {
    MemorySpace(&'a BaseAddressRegister<MemorySpace>),
    IOSpace(&'a BaseAddressRegister<IOSpace>),
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySpaceAddressType {
    Bit32 = 0b00,
    Bit64 = 0b10,
    Reserved = 0b01,
    Invalid = 0b11,
}

#[repr(transparent)]
pub struct BaseAddressRegister<T: BARLayoutVariant> {
    value: u32,
    phantom: core::marker::PhantomData<T>,
}

impl BaseAddressRegister<MemorySpace> {
    pub fn base_address<T>(&self) -> *const T {
        (self.value & !0b1111) as *const _
    }

    pub fn prefetchable(&self) -> bool {
        (self.value & (1 << 3)) > 0
    }

    pub fn address_type(&self) -> MemorySpaceAddressType {
        match (self.value >> 1) & 0b11 {
            0 => MemorySpaceAddressType::Bit32,
            1 => MemorySpaceAddressType::Reserved,
            2 => MemorySpaceAddressType::Bit64,
            _ => MemorySpaceAddressType::Invalid,
        }
    }
}

impl BaseAddressRegister<IOSpace> {
    pub fn base_address<T>(&self) -> *const T {
        (self.value & !0b11) as *const _
    }
}

impl fmt::Debug for BaseAddressRegister<MemorySpace> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BaseAddressRegister<MemorySpace>")
            .field("Base Address", &self.base_address::<u8>())
            .field("Prefetchable", &self.prefetchable())
            .field("Type", &self.address_type())
            .finish()
    }
}

impl fmt::Debug for BaseAddressRegister<IOSpace> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BaseAddressRegister<IOSpace>")
            .field(&self.base_address::<u8>())
            .finish()
    }
}

pub trait ExtPCIDeviceVariant {}

pub enum Standard {}
impl ExtPCIDeviceVariant for Standard {}

pub enum PCI2PCI {}
impl ExtPCIDeviceVariant for PCI2PCI {}

pub enum PCI2CardBus {}
impl ExtPCIDeviceVariant for PCI2CardBus {}

#[derive(Debug)]
pub enum ExtPCIDeviceType<'a> {
    Standard(&'a ExtPCIDeviceHeader<Standard>),
    PCI2PCI(&'a ExtPCIDeviceHeader<PCI2PCI>),
    PCI2CardBus(&'a ExtPCIDeviceHeader<PCI2CardBus>),
}

pub struct ExtPCIDeviceHeader<T: ExtPCIDeviceVariant> {
    phantom: core::marker::PhantomData<T>,
}

impl ExtPCIDeviceHeader<Standard> {
    unsafe fn offset<T: Copy>(&self, offset: usize) -> &T {
        &*((self as *const _ as *const u8).add(offset) as *const T)
    }

    unsafe fn base_addr_register(&self, offset: usize) -> BaseAddressRegisterType {
        let register = (self as *const _ as *const u8).add(offset) as *const u32;

        match (*register & 0b1) > 0 {
            false => BaseAddressRegisterType::MemorySpace(
                &*(register as *const BaseAddressRegister<MemorySpace>),
            ),
            true => BaseAddressRegisterType::IOSpace(
                &*(register as *const BaseAddressRegister<IOSpace>),
            ),
        }
    }

    pub fn bar_0(&self) -> BaseAddressRegisterType {
        unsafe { self.base_addr_register(0x10) }
    }

    pub fn bar_1(&self) -> BaseAddressRegisterType {
        unsafe { self.base_addr_register(0x14) }
    }

    pub fn bar_2(&self) -> BaseAddressRegisterType {
        unsafe { self.base_addr_register(0x18) }
    }

    pub fn bar_3(&self) -> BaseAddressRegisterType {
        unsafe { self.base_addr_register(0x1C) }
    }

    pub fn bar_4(&self) -> BaseAddressRegisterType {
        unsafe { self.base_addr_register(0x20) }
    }

    pub fn bar_5(&self) -> BaseAddressRegisterType {
        unsafe { self.base_addr_register(0x24) }
    }

    pub fn cardbus_cis_ptr(&self) -> u32 {
        unsafe { *self.offset(0x28) }
    }

    pub fn subsystem_vendor_id(&self) -> u16 {
        unsafe { *self.offset(0x2C) }
    }

    pub fn subsystem_id(&self) -> u16 {
        unsafe { *self.offset(0x2E) }
    }

    pub fn expansion_rom_base_addr(&self) -> u32 {
        unsafe { *self.offset(0x30) }
    }

    pub fn capabilities_ptr(&self) -> u8 {
        unsafe { *self.offset::<u8>(0x34) & !0b11 }
    }

    pub fn interrupt_line(&self) -> Option<u8> {
        match unsafe { self.offset(0x3C) } {
            0xFF => None,
            value => Some(*value),
        }
    }

    pub fn interrupt_pin(&self) -> Option<u8> {
        match unsafe { self.offset(0x3D) } {
            0x0 => None,
            value => Some(*value),
        }
    }

    pub fn min_grant(&self) -> u8 {
        unsafe { *self.offset(0x3E) }
    }

    pub fn max_latency(&self) -> u8 {
        unsafe { *self.offset(0x3F) }
    }
}

impl fmt::Debug for ExtPCIDeviceHeader<Standard> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Extended PCI Device Header")
            .field("Base Address Register 0", &self.bar_0())
            .field("Base Address Register 1", &self.bar_1())
            .field("Base Address Register 2", &self.bar_2())
            .field("Base Address Register 3", &self.bar_3())
            .field("Base Address Register 4", &self.bar_4())
            .field("Base Address Register 5", &self.bar_5())
            .field("Cardbus CIS Pointer", &self.cardbus_cis_ptr())
            .field("Subsystem Vendor ID", &self.subsystem_vendor_id())
            .field("Subsystem ID", &self.subsystem_id())
            .field(
                "Expansion ROM Base Address",
                &self.expansion_rom_base_addr(),
            )
            .field("Capabilities Pointer", &self.capabilities_ptr())
            .field("Interrupt Line", &self.interrupt_line())
            .field("Interrupt Pin", &self.interrupt_pin())
            .field("Min Grant", &self.min_grant())
            .field("Max Latency", &self.max_latency())
            .finish()
    }
}

impl fmt::Debug for ExtPCIDeviceHeader<PCI2PCI> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ss").finish()
    }
}
impl fmt::Debug for ExtPCIDeviceHeader<PCI2CardBus> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ss").finish()
    }
}
