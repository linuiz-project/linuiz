use crate::{memory::Frame, registers::MSR, structures::GUID};
use core::marker::PhantomData;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICRegister {
    ID = 0x20,
    Version = 0x30,
    TaskPriority = 0x80,
    LDR = 0xD0,
    DFR = 0xE0,
    Spurious = 0xF0,
    ESR = 0x280,
    ICRL = 0x300,
    ICRH = 0x310,
    TimerInitialCount = 0x380,
    TimeCurrentCount = 0x390,
    TimerDivisor = 0x3E0,
    Last = 0x38F,
    TimerBaseDivisor = 1 << 20,
    LVT_TIMER = 0x320,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICTimerMode {
    OneShot = 0b00,
    Periodic = 0b01,
    TSC_Deadline = 0b10,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICTimerDivisor {
    Div2 = 0b0000,
    Div4 = 0b0001,
    Div8 = 0b0010,
    Div16 = 0b0011,
    Div32 = 0b1000,
    Div64 = 0b1001,
    Div128 = 0b1010,
    Div1 = 0b1011,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICDeliveryMode {
    Fixed = 0b000,
    SystemManagement = 0b010,
    NonMaskable = 0b100,
    External = 0b111,
    INIT = 0b101,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum APICInterruptRegister {
    CMCI,
    Timer,
    ThermalSensor,
    PerformanceCounter,
    LINT0,
    LINT1,
    Error,
}

pub struct APIC {
    mmio: crate::memory::MMIO,
}

impl APIC {
    const LVT_CMCI: usize = 0x2F0;
    const LVT_TIMER: usize = 0x320;
    const LVT_THERMAL_SENSOR: usize = 0x330;
    const LVT_PERFORMANCE: usize = 0x340;
    const LVT_LINT0: usize = 0x350;
    const LVT_LINT1: usize = 0x360;
    const LVT_ERROR: usize = 0x370;

    // The `mask` bit for an LVT entry.
    pub const DISABLE: u32 = 0x10000;
    pub const SW_ENABLE: u32 = 0x100;
    pub const CPU_FOCUS: u32 = 0x200;

    pub fn mmio_addr() -> PhysAddr {
        PhysAddr::new(MSR::IA32_APIC_BASE.read() & !0xFFF)
    }

    pub fn mmio_frames() -> crate::memory::FrameIterator {
        Frame::range_count(Self::mmio_addr(), 1)
    }

    pub fn from_msr(mapped_addr: x86_64::VirtAddr) -> Self {
        Self {
            mmio: unsafe {
                crate::memory::MMIO::new(Self::mmio_addr(), mapped_addr, 0x1000).unwrap()
            },
        }
    }

    #[inline]
    pub fn signal_eoi(&mut self) {
        const EOI_REGISTER: usize = 0xB0;

        debug!(".");
        self.mmio.write(EOI_REGISTER, 0);
    }

    #[inline]
    pub fn cmci(&mut self) -> LVTRegister<Generic> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_CMCI).unwrap())
    }

    #[inline]
    pub fn timer(&mut self) -> LVTRegister<Timer> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_TIMER).unwrap())
    }

    #[inline]
    pub fn lint0(&mut self) -> LVTRegister<LINT> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_LINT0).unwrap())
    }

    #[inline]
    pub fn lint1(&mut self) -> LVTRegister<LINT> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_LINT1).unwrap())
    }

    #[inline]
    pub fn error(&mut self) -> LVTRegister<Error> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_ERROR).unwrap())
    }

    #[inline]
    pub fn performance(&mut self) -> LVTRegister<Generic> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_PERFORMANCE).unwrap())
    }

    #[inline]
    pub fn thermal_sensor(&mut self) -> LVTRegister<Generic> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_THERMAL_SENSOR).unwrap())
    }
}

impl core::ops::Index<APICRegister> for APIC {
    type Output = u32;

    #[inline]
    fn index(&self, register: APICRegister) -> &Self::Output {
        self.mmio.read(register as usize).unwrap()
    }
}

impl core::ops::IndexMut<APICRegister> for APIC {
    #[inline]
    fn index_mut(&mut self, register: APICRegister) -> &mut Self::Output {
        self.mmio.read_mut(register as usize).unwrap()
    }
}

pub trait LVTRegisterVariant {}

pub enum Timer {}
impl LVTRegisterVariant for Timer {}

pub enum Generic {}
impl LVTRegisterVariant for Generic {}

pub enum LINT {}
impl LVTRegisterVariant for LINT {}

pub enum Error {}
impl LVTRegisterVariant for Error {}

use bit_field::BitField;

#[repr(transparent)]
pub struct LVTRegister<'val, T: LVTRegisterVariant + ?Sized> {
    value: &'val mut u32,
    phantom: PhantomData<T>,
}

impl<'val, T: LVTRegisterVariant> LVTRegister<'val, T> {
    #[inline]
    fn new(value: &'val mut u32) -> Self {
        Self {
            value,
            phantom: PhantomData,
        }
    }

    #[inline]
    pub fn is_interrupted(&self) -> bool {
        self.value.get_bit(12)
    }

    #[inline]
    pub fn is_masked(&self) -> bool {
        self.value.get_bit(16)
    }

    #[inline]
    pub fn set_masked(&mut self, masked: bool) {
        self.value.set_bit(16, masked);
    }

    #[inline]
    pub fn get_vector(&self) -> u8 {
        self.value.get_bits(0..8) as u8
    }

    #[inline]
    pub fn set_vector(&mut self, vector: u8) {
        self.value.set_bits(0..8, vector as u32);
    }
}

impl LVTRegister<'_, Timer> {
    #[inline]
    pub fn set_mode(&mut self, mode: APICTimerMode) {
        self.value.set_bits(17..19, mode as u32);
    }
}

impl LVTRegister<'_, Generic> {
    #[inline]
    pub fn set_delivery_mode(&mut self, mode: APICDeliveryMode) {
        self.value.set_bits(8..11, mode as u32);
    }
}
