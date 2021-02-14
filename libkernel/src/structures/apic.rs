use crate::{registers::MSR, structures::GUID};
use x86_64::PhysAddr;

pub const ACPI_GUID: GUID = GUID::new(
    0xeb9d2d30,
    0x2d88,
    0x11d3,
    0x9a16,
    [0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
);

pub const ACPI2_GUID: GUID = GUID::new(
    0x8868e871,
    0xe4f1,
    0x11d3,
    0xbc22,
    [0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
);

#[repr(u32)]
pub enum APICRegister {
    ID = 0x20,
    Version = 0x30,
    TaskPriority = 0x80,
    EndOfInterrupt = 0xB0,
    LDR = 0xD0,
    DFR = 0xE0,
    Spurious = 0xF0,
}

pub struct APIC {
    base_addr: PhysAddr,
}

impl APIC {
    pub fn from_msr() -> Self {
        Self {
            base_addr: PhysAddr::new(MSR::IA32_APIC_BASE.read() & !(0xFFF)),
        }
    }

    pub unsafe fn from_addr(base_addr: PhysAddr) -> Self {
        Self { base_addr }
    }

    pub fn addr(&self) -> PhysAddr {
        self.base_addr
    }
}

impl core::ops::Index<APICRegister> for APIC {
    type Output = u128;

    fn index(&self, register: APICRegister) -> &Self::Output {
        let offset = register as u64;
        let offset_addr = (self.base_addr + offset).as_u64() as *const u128;
        unsafe { &*offset_addr }
    }
}
