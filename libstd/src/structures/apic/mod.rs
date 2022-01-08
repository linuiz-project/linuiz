pub mod icr;

use crate::{
    addr_ty::Virtual,
    memory::{volatile::VolatileCell, MMIO},
    registers::MSR,
    Address, ReadWrite,
};
use core::marker::PhantomData;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Register {
    ID = 0x20,
    Version = 0x30,
    TaskPriority = 0x80,
    LDR = 0xD0,
    DFR = 0xE0,
    Spurious = 0xF0,
    ICRL = 0x300,
    ICRH = 0x310,
    TimerInitialCount = 0x380,
    TimerCurrentCount = 0x390,
    TimerDivisor = 0x3E0,
    Last = 0x38F,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum TimerMode {
    OneShot = 0b00,
    Periodic = 0b01,
    TSC_Deadline = 0b10,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum TimerDivisor {
    Div2 = 0b0000,
    Div4 = 0b0001,
    Div8 = 0b0010,
    Div16 = 0b0011,
    Div32 = 0b1000,
    Div64 = 0b1001,
    Div128 = 0b1010,
    Div1 = 0b1011,
}

impl TimerDivisor {
    pub const fn as_divide_value(self) -> u32 {
        match self {
            TimerDivisor::Div2 => 2,
            TimerDivisor::Div4 => 4,
            TimerDivisor::Div8 => 8,
            TimerDivisor::Div16 => 16,
            TimerDivisor::Div32 => 32,
            TimerDivisor::Div64 => 64,
            TimerDivisor::Div128 => 128,
            TimerDivisor::Div1 => 1,
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum DeliveryMode {
    Fixed = 0b000,
    SystemManagement = 0b010,
    NonMaskable = 0b100,
    External = 0b111,
    INIT = 0b101,
}

bitflags::bitflags! {
    pub struct ErrorStatusFlags: u8 {
        const SEND_CHECKSUM_ERROR = 1 << 0;
        const RECEIVE_CHECKSUM_ERROR = 1 << 1;
        const SEND_ACCEPT_ERROR = 1 << 2;
        const RECEIVE_ACCEPT_ERROR = 1 << 3;
        const REDIRECTABLE_IPI = 1 << 4;
        const SENT_ILLEGAL_VECTOR = 1 << 5;
        const RECEIVED_ILLEGAL_VECTOR = 1 << 6;
        const ILLEGAL_REGISTER_ADDRESS = 1 << 7;
    }
}

#[repr(u8)]
#[derive(Debug)]
pub enum InterruptVector {
    Timer = 32,
    CMCI,
    Performance,
    ThermalSensor,
    LINT0,
    LINT1,
    Error,
    Storage,
    Spurious = u8::MAX,
}

pub struct APIC(MMIO);

impl APIC {
    pub fn from_msr() -> Self {
        unsafe {
            Self::new(
                crate::memory::MMIO::new(MSR::IA32_APIC_BASE.read().get_bits(12..36) as usize, 1)
                    .expect("Allocation failure when attempting to create MMIO for APIC"),
            )
        }
    }

    pub unsafe fn new(mmio: MMIO) -> Self {
        let assumed_base = MSR::IA32_APIC_BASE.read().get_bits(12..36) as usize;
        if assumed_base != mmio.frames().start {
            warn!(
                "APIC MMIO BASE FRAME INDEX CHECK: IA32_APIC_BASE({:?}) != PROVIDED({:?})",
                assumed_base,
                mmio.frames().start
            );
        }

        let apic = Self(mmio);

        apic.timer().set_vector(InterruptVector::Timer as u8);
        apic.cmci().set_vector(InterruptVector::CMCI as u8);
        apic.performance()
            .set_vector(InterruptVector::Performance as u8);
        apic.thermal_sensor()
            .set_vector(InterruptVector::ThermalSensor as u8);
        apic.lint0().set_vector(InterruptVector::LINT0 as u8);
        apic.lint1().set_vector(InterruptVector::LINT1 as u8);
        apic.error().set_vector(InterruptVector::Error as u8);
        apic.write_register(
            Register::Spurious,
            *apic
                .read_register(Register::Spurious)
                .set_bits(0..8, InterruptVector::Spurious as u32),
        );

        apic
    }

    pub unsafe fn mapped_addr(&self) -> Address<Virtual> {
        self.0.mapped_addr()
    }

    pub fn is_enabled(&self) -> bool {
        (MSR::IA32_APIC_BASE.read() & (1 << 11)) > 0
    }

    pub unsafe fn hw_enable(&self) {
        MSR::IA32_APIC_BASE.write_bit(11, true);
    }

    pub unsafe fn hw_disable(&self) {
        MSR::IA32_APIC_BASE.write_bit(11, false);
    }

    pub fn sw_enable(&self) {
        self.write_register(
            Register::Spurious,
            *self.read_register(Register::Spurious).set_bit(9, true),
        );
    }

    pub fn sw_disable(&self) {
        self.write_register(
            Register::Spurious,
            *self.read_register(Register::Spurious).set_bit(9, false),
        );
    }

    pub fn set_eoi_broadcast_suppression(&self, suppress: bool) -> Result<(), ()> {
        if self.read_register(Register::Version).get_bit(24) {
            self.write_register(
                Register::Spurious,
                *self.read_register(Register::Spurious).set_bit(12, suppress),
            );

            Ok(())
        } else {
            Err(())
        }
    }

    pub fn id(&self) -> u8 {
        self.read_register(Register::ID).get_bits(24..) as u8
    }

    pub fn version(&self) -> u8 {
        self.read_register(Register::Version).get_bits(0..8) as u8
    }

    pub fn max_lvt_entry(&self) -> u8 {
        self.read_register(Register::Version).get_bits(16..24) as u8
    }

    pub fn error_status(&self) -> ErrorStatusFlags {
        ErrorStatusFlags::from_bits_truncate(unsafe { self.0.read(0x280).assume_init() })
    }

    pub fn read_register(&self, register: Register) -> u32 {
        unsafe { self.0.read(register as usize).assume_init() }
    }

    pub fn write_register(&self, register: Register, value: u32) {
        unsafe { self.0.write(register as usize, value) }
    }

    pub fn end_of_interrupt(&self) {
        unsafe { self.0.write(0xB0, 0) };
    }

    pub fn cmci(&self) -> LVTRegister<Generic> {
        unsafe {
            LVTRegister::<Generic> {
                obj: self.0.borrow::<VolatileCell<u32, ReadWrite>>(0x2F0),
                phantom: PhantomData,
            }
        }
    }

    pub fn timer(&self) -> LVTRegister<Timer> {
        unsafe {
            LVTRegister::<Timer> {
                obj: self.0.borrow::<VolatileCell<u32, ReadWrite>>(0x320),
                phantom: PhantomData,
            }
        }
    }

    pub fn thermal_sensor(&self) -> LVTRegister<Generic> {
        unsafe {
            LVTRegister::<Generic> {
                obj: self.0.borrow::<VolatileCell<u32, ReadWrite>>(0x330),
                phantom: PhantomData,
            }
        }
    }

    pub fn performance(&self) -> LVTRegister<Generic> {
        unsafe {
            LVTRegister::<Generic> {
                obj: self.0.borrow::<VolatileCell<u32, ReadWrite>>(0x340),
                phantom: PhantomData,
            }
        }
    }

    pub fn lint0(&self) -> LVTRegister<LINT> {
        unsafe {
            LVTRegister::<LINT> {
                obj: self.0.borrow::<VolatileCell<u32, ReadWrite>>(0x350),
                phantom: PhantomData,
            }
        }
    }

    pub fn lint1(&self) -> LVTRegister<LINT> {
        unsafe {
            LVTRegister::<LINT> {
                obj: self.0.borrow::<VolatileCell<u32, ReadWrite>>(0x360),
                phantom: PhantomData,
            }
        }
    }

    pub fn error(&self) -> LVTRegister<Error> {
        unsafe {
            LVTRegister::<Error> {
                obj: self.0.borrow::<VolatileCell<u32, ReadWrite>>(0x370),
                phantom: PhantomData,
            }
        }
    }

    pub fn interrupt_command_register(&self) -> icr::InterruptCommandRegister {
        unsafe {
            icr::InterruptCommandRegister::new(
                self.0.borrow(Register::ICRL as usize),
                self.0.borrow(Register::ICRH as usize),
            )
        }
    }

    pub unsafe fn reset(&self) {
        self.sw_disable();
        self.write_register(Register::DFR, 0xFFFFFFFF);
        let mut ldr = self.read_register(Register::LDR);
        ldr &= 0xFFFFFF;
        ldr = (ldr & !0xFF) | ((ldr & 0xFF) | 1);
        self.write_register(Register::LDR, ldr);
        self.timer().set_masked(true);
        self.performance()
            .set_delivery_mode(DeliveryMode::NonMaskable);
        // self.lint0().set_masked(true);
        self.lint1().set_masked(true);
        self.write_register(Register::TaskPriority, 0);
        self.write_register(Register::TimerInitialCount, 0);
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
pub struct LVTRegister<'reg, T: LVTRegisterVariant> {
    obj: &'reg crate::memory::volatile::VolatileCell<u32, ReadWrite>,
    phantom: PhantomData<T>,
}

impl<T: LVTRegisterVariant> LVTRegister<'_, T> {
    const INTERRUPTED_OFFSET: usize = 12;
    const MASKED_OFFSET: usize = 16;
    const VECTOR_MASK: u32 = 0xFF;

    pub fn get_interrupted(&self) -> bool {
        self.obj.read().get_bit(Self::INTERRUPTED_OFFSET)
    }

    pub fn is_masked(&self) -> bool {
        self.obj.read().get_bit(Self::MASKED_OFFSET)
    }

    pub fn set_masked(&mut self, masked: bool) {
        self.obj
            .write(*self.obj.read().set_bit(Self::MASKED_OFFSET, masked));
    }

    pub fn get_vector(&self) -> u8 {
        (self.obj.read() & Self::VECTOR_MASK) as u8
    }

    fn set_vector(&mut self, vector: u8) {
        self.obj
            .write((self.obj.read() & !Self::VECTOR_MASK) | vector as u32);
    }
}

impl LVTRegister<'_, Timer> {
    pub fn set_mode(&mut self, mode: TimerMode) {
        self.obj
            .write(*self.obj.read().set_bits(17..19, mode as u32));
    }
}

impl LVTRegister<'_, Generic> {
    pub fn set_delivery_mode(&mut self, mode: DeliveryMode) {
        self.obj
            .write(*self.obj.read().set_bits(8..11, mode as u32));
    }
}
