use crate::{
    io::pci::PCIDeviceHeader,
    memory::mmio::{Mapped, MMIO},
};
use core::fmt;

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

pub trait PCIeDeviceVariant {}

pub enum Standard {}
impl PCIeDeviceVariant for Standard {}

pub enum PCI2PCI {}
impl PCIeDeviceVariant for PCI2PCI {}

pub enum PCI2CardBus {}
impl PCIeDeviceVariant for PCI2CardBus {}

#[derive(Debug)]
pub enum PCIeDeviceType<'a> {
    Standard(&'a PCIeDeviceHeader<Standard>),
    PCI2PCI(&'a PCIeDeviceHeader<PCI2PCI>),
    PCI2CardBus(&'a PCIeDeviceHeader<PCI2CardBus>),
}

pub struct PCIeDeviceHeader<T: PCIeDeviceVariant> {
    phantom: core::marker::PhantomData<T>,
}

impl PCIeDeviceHeader<Standard> {
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

impl fmt::Debug for PCIeDeviceHeader<Standard> {
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

impl fmt::Debug for PCIeDeviceHeader<PCI2PCI> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ss").finish()
    }
}

impl fmt::Debug for PCIeDeviceHeader<PCI2CardBus> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ss").finish()
    }
}

pub struct PCIeDevice {
    mmio: MMIO<Mapped>,
}

impl PCIeDevice {
    pub const fn new(mmio: MMIO<Mapped>) -> Self {
        Self { mmio }
    }

    pub fn base_header(&self) -> &PCIDeviceHeader {
        unsafe { self.mmio.read(0).unwrap() }
    }

    pub fn ext_header(&self) -> PCIeDeviceType {
        match self.base_header().header_type() {
            0x0 => PCIeDeviceType::Standard(unsafe { self.mmio.read(0).unwrap() }),
            0x1 => PCIeDeviceType::PCI2PCI(unsafe { self.mmio.read(0).unwrap() }),
            0x2 => PCIeDeviceType::PCI2CardBus(unsafe { self.mmio.read(0).unwrap() }),
            header_type => panic!("invalid header type: 0x{:X}", header_type),
        }
    }

    // TODO remove this
    pub fn consume_mmio(self) -> MMIO<Mapped> {
        self.mmio
    }
}

impl core::fmt::Debug for PCIeDevice {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PCIe Device")
            .field("Header", self.base_header())
            .field("Extended Header", &self.ext_header())
            .finish()
    }
}
