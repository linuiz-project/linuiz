use crate::{memory::Frame, registers::MSR};
use core::{lazy::OnceCell, marker::PhantomData};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum LocalAPICRegister {
    ID = 0x20,
    Version = 0x30,
    TaskPriority = 0x80,
    LDR = 0xD0,
    DFR = 0xE0,
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
pub enum LocalAPICTimerMode {
    OneShot = 0b00,
    Periodic = 0b01,
    TSC_Deadline = 0b10,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum LocalAPICTimerDivisor {
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
pub enum LocalAPICDeliveryMode {
    Fixed = 0b000,
    SystemManagement = 0b010,
    NonMaskable = 0b100,
    External = 0b111,
    INIT = 0b101,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalAPICInterruptRegister {
    CMCI,
    Timer,
    ThermalSensor,
    PerformanceCounter,
    LINT0,
    LINT1,
    Error,
}

pub struct LocalAPIC {
    mmio: crate::memory::MMIO,
}

impl LocalAPIC {
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

    pub fn mmio_addr() -> x86_64::PhysAddr {
        x86_64::PhysAddr::new(MSR::IA32_APIC_BASE.read() & !0xFFF)
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

    pub unsafe fn enable(&mut self) {
        MSR::IA32_APIC_BASE.write_bit(11, true);
    }

    pub unsafe fn disable(&mut self) {
        MSR::IA32_APIC_BASE.write_bit(11, false);
    }

    pub fn signal_eoi(&mut self) {
        const EOI_REGISTER: usize = 0xB0;

        debug!(".");
        self.mmio.write(EOI_REGISTER, 0);
    }

    pub fn cmci(&mut self) -> LVTRegister<Generic> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_CMCI).unwrap())
    }

    pub fn timer(&mut self) -> LVTRegister<Timer> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_TIMER).unwrap())
    }

    pub fn lint0(&mut self) -> LVTRegister<LINT> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_LINT0).unwrap())
    }

    pub fn lint1(&mut self) -> LVTRegister<LINT> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_LINT1).unwrap())
    }

    pub fn error(&mut self) -> LVTRegister<Error> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_ERROR).unwrap())
    }

    pub fn performance(&mut self) -> LVTRegister<Generic> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_PERFORMANCE).unwrap())
    }

    pub fn thermal_sensor(&mut self) -> LVTRegister<Generic> {
        LVTRegister::new(self.mmio.read_mut(Self::LVT_THERMAL_SENSOR).unwrap())
    }

    pub fn configure_spurious(&mut self, vector: u8, enabled: bool) {
        const LVT_SPURIOUS: usize = 0xF0;

        let spurious = self.mmio.read_mut::<u32>(LVT_SPURIOUS).unwrap();
        spurious.set_bits(0..8, vector as u32);
        spurious.set_bit(8, enabled);
    }
}

impl core::ops::Index<LocalAPICRegister> for LocalAPIC {
    type Output = u32;

    fn index(&self, register: LocalAPICRegister) -> &Self::Output {
        self.mmio.read(register as usize).unwrap()
    }
}

impl core::ops::IndexMut<LocalAPICRegister> for LocalAPIC {
    fn index_mut(&mut self, register: LocalAPICRegister) -> &mut Self::Output {
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
    fn new(value: &'val mut u32) -> Self {
        Self {
            value,
            phantom: PhantomData,
        }
    }

    pub fn is_interrupted(&self) -> bool {
        self.value.get_bit(12)
    }

    pub fn is_masked(&self) -> bool {
        self.value.get_bit(16)
    }

    pub fn set_masked(&mut self, masked: bool) {
        self.value.set_bit(16, masked);
    }

    pub fn get_vector(&self) -> u8 {
        self.value.get_bits(0..8) as u8
    }

    pub fn set_vector(&mut self, vector: u8) {
        self.value.set_bits(0..8, vector as u32);
    }
}

impl LVTRegister<'_, Timer> {
    pub fn set_mode(&mut self, mode: LocalAPICTimerMode) {
        self.value.set_bits(17..19, mode as u32);
    }
}

impl LVTRegister<'_, Generic> {
    #[inline]
    pub fn set_delivery_mode(&mut self, mode: LocalAPICDeliveryMode) {
        self.value.set_bits(8..11, mode as u32);
    }
}

static LOCAL_APIC: spin::Mutex<OnceCell<LocalAPIC>> = spin::Mutex::new(OnceCell::new());

#[cfg(feature = "kernel_impls")]
pub fn load() {
    debug!("Loading local APIC table.");
    let mapped_addr =
        x86_64::VirtAddr::from_ptr(unsafe { crate::memory::alloc_to(LocalAPIC::mmio_frames()) });
    debug!("Allocated APIC table virtual address: {:?}", mapped_addr);

    if LOCAL_APIC
        .lock()
        .set(LocalAPIC::from_msr(mapped_addr))
        .is_err()
    {
        panic!("local APIC has already been loaded")
    }
}

pub fn reset() {
    local_apic_mut(|lapic_option| match lapic_option {
        Some(lapic) => {
            debug!("Initializing APIC to known state.");
            lapic[LocalAPICRegister::DFR] = 0xFFFFFFFF;
            let mut ldr = lapic[LocalAPICRegister::LDR];
            ldr &= 0xFFFFFF;
            ldr = (ldr & !0xFF) | ((ldr & 0xFF) | 1);
            lapic[LocalAPICRegister::LDR] = ldr;

            debug!("Masking all LVT interrupts.");
            lapic.timer().set_masked(true);
            lapic
                .performance()
                .set_delivery_mode(LocalAPICDeliveryMode::NonMaskable);
            lapic.lint0().set_masked(true);
            lapic.lint1().set_masked(true);
            //lapic[LocalAPICRegister::TaskPriority] = 0;
        }
        None => panic!("local APIC has not been loaded"),
    });
}

pub fn local_apic<F, R>(callback: F) -> R
where
    F: FnOnce(Option<&LocalAPIC>) -> R,
{
    crate::instructions::interrupts::without_interrupts(|| callback(LOCAL_APIC.lock().get()))
}

pub fn local_apic_mut<F, R>(callback: F) -> R
where
    F: FnOnce(Option<&mut LocalAPIC>) -> R,
{
    crate::instructions::interrupts::without_interrupts(|| callback(LOCAL_APIC.lock().get_mut()))
}
