use crate::{
    addr_ty::Virtual,
    io::pci::{PCIeDevice, Standard},
    Address,
};
use core::fmt;

pub trait BaseAddressRegisterType {
    fn base_address<T>(raw: u32) -> *const T;
}

pub enum MemorySpace {}
impl BaseAddressRegisterType for MemorySpace {
    fn base_address<T>(raw: u32) -> *const T {
        (raw & !0b1111) as *const _
    }
}

pub enum IOSpace {}
impl BaseAddressRegisterType for IOSpace {
    fn base_address<T>(raw: u32) -> *const T {
        (raw & !0b11) as *const _
    }
}

#[derive(Debug)]
pub enum BaseAddressRegisterVariant<'a> {
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
pub struct BaseAddressRegister<V: BaseAddressRegisterType> {
    value: u32,
    phantom: core::marker::PhantomData<V>,
}

impl<V: BaseAddressRegisterType> BaseAddressRegister<V> {
    pub fn base_address(&self) -> Address<Virtual> {
        Address::<Virtual>::from_ptr(V::base_address::<u8>(self.value))
    }
}

impl BaseAddressRegister<MemorySpace> {
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

impl fmt::Debug for BaseAddressRegister<MemorySpace> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BaseAddressRegister<MemorySpace>")
            .field("Base Address", &self.base_address())
            .field("Prefetchable", &self.prefetchable())
            .field("Type", &self.address_type())
            .finish()
    }
}

impl fmt::Debug for BaseAddressRegister<IOSpace> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BaseAddressRegister<IOSpace>")
            .field(&self.base_address())
            .finish()
    }
}

impl PCIeDevice<Standard> {
    unsafe fn base_addr_register(&self, offset: usize) -> Option<BaseAddressRegisterVariant> {
        const REGISTER_BASE: u32 = 0x10;
        const REGISTER_OFFSET: u32 = 0x4;

        let raw = self.mmio.read::<u32>(offset).unwrap();

        if (*raw & 0b1) == 0 {
            let register = &*(raw as *const _ as *const BaseAddressRegister<MemorySpace>);

            if register.base_address() != Address::zero() {
                Some(BaseAddressRegisterVariant::MemorySpace(register))
            } else {
                None
            }
        } else {
            let register = &*(raw as *const _ as *const BaseAddressRegister<IOSpace>);

            if register.base_address() != Address::zero() {
                Some(BaseAddressRegisterVariant::IOSpace(register))
            } else {
                None
            }
        }
    }

    pub fn register0(&self) -> Option<BaseAddressRegisterVariant> {
        unsafe { self.base_addr_register(0x10) }
    }

    pub fn register1(&self) -> Option<BaseAddressRegisterVariant> {
        unsafe { self.base_addr_register(0x14) }
    }

    pub fn register2(&self) -> Option<BaseAddressRegisterVariant> {
        unsafe { self.base_addr_register(0x18) }
    }

    pub fn register3(&self) -> Option<BaseAddressRegisterVariant> {
        unsafe { self.base_addr_register(0x1C) }
    }

    pub fn register4(&self) -> Option<BaseAddressRegisterVariant> {
        unsafe { self.base_addr_register(0x20) }
    }

    pub fn register5(&self) -> Option<BaseAddressRegisterVariant> {
        unsafe { self.base_addr_register(0x24) }
    }

    pub fn cardbus_cis_ptr(&self) -> &u32 {
        unsafe { *self.mmio.read(0x28).unwrap() }
    }

    pub fn subsystem_vendor_id(&self) -> u16 {
        unsafe { *self.mmio.read(0x2C).unwrap() }
    }

    pub fn subsystem_id(&self) -> u16 {
        unsafe { *self.mmio.read(0x2E).unwrap() }
    }

    pub fn expansion_rom_base_addr(&self) -> u32 {
        unsafe { *self.mmio.read(0x30).unwrap() }
    }

    pub fn capabilities_ptr(&self) -> u8 {
        unsafe { *self.mmio.read::<u8>(0x34).unwrap() & !0b11 }
    }

    pub fn interrupt_line(&self) -> Option<u8> {
        match unsafe { *self.mmio.read(0x3C).unwrap() } {
            0xFF => None,
            value => Some(value),
        }
    }

    pub fn interrupt_pin(&self) -> Option<u8> {
        match unsafe { *self.mmio.read(0x3D).unwrap() } {
            0x0 => None,
            value => Some(value),
        }
    }

    pub fn min_grant(&self) -> u8 {
        unsafe { *self.mmio.read(0x3E).unwrap() }
    }

    pub fn max_latency(&self) -> u8 {
        unsafe { *self.mmio.read(0x3F).unwrap() }
    }
}

impl fmt::Debug for PCIeDevice<Standard> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PCIe Device (Standard)")
            .field("Base Address Register 0", &self.register0())
            .field("Base Address Register 1", &self.register1())
            .field("Base Address Register 2", &self.register2())
            .field("Base Address Register 3", &self.register3())
            .field("Base Address Register 4", &self.register4())
            .field("Base Address Register 5", &self.register5())
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
