pub mod icr;

use crate::{
    addr_ty::Virtual,
    memory::{
        mmio::{Mapped, MMIO},
        volatile::VolatileCell,
    },
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
    SPR = 0xF0,
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

pub struct APIC {
    mmio: MMIO<Mapped>,
}

impl APIC {
    pub unsafe fn new(mmio: MMIO<Mapped>) -> Self {
        debug!(
            "Creating APIC: {:?} -> {:?}",
            mmio.mapped_addr(),
            mmio.frames().start().base_addr()
        );

        Self { mmio }
    }

    pub unsafe fn mapped_addr(&self) -> Address<Virtual> {
        self.mmio.mapped_addr()
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
            Register::SPR,
            *self.read_register(Register::SPR).set_bit(9, true),
        );
    }

    pub fn sw_disable(&self) {
        self.write_register(
            Register::SPR,
            *self.read_register(Register::SPR).set_bit(9, false),
        );
    }

    pub fn id(&self) -> u8 {
        self.read_register(Register::ID) as u8
    }

    pub fn end_of_interrupt(&self) {
        unsafe { self.mmio.write(0xB0, 0).unwrap() };
    }

    pub fn cmci(&self) -> LVTRegister<Generic> {
        unsafe {
            LVTRegister::<Generic> {
                obj: self
                    .mmio
                    .borrow::<VolatileCell<u32, ReadWrite>>(0x2F0)
                    .unwrap(),
                phantom: PhantomData,
            }
        }
    }

    pub fn timer(&self) -> LVTRegister<Timer> {
        unsafe {
            LVTRegister::<Timer> {
                obj: self
                    .mmio
                    .borrow::<VolatileCell<u32, ReadWrite>>(0x320)
                    .unwrap(),
                phantom: PhantomData,
            }
        }
    }

    pub fn thermal_sensor(&self) -> LVTRegister<Generic> {
        unsafe {
            LVTRegister::<Generic> {
                obj: self
                    .mmio
                    .borrow::<VolatileCell<u32, ReadWrite>>(0x330)
                    .unwrap(),
                phantom: PhantomData,
            }
        }
    }

    pub fn performance(&self) -> LVTRegister<Generic> {
        unsafe {
            LVTRegister::<Generic> {
                obj: self
                    .mmio
                    .borrow::<VolatileCell<u32, ReadWrite>>(0x340)
                    .unwrap(),
                phantom: PhantomData,
            }
        }
    }

    pub fn lint0(&self) -> LVTRegister<LINT> {
        unsafe {
            LVTRegister::<LINT> {
                obj: self
                    .mmio
                    .borrow::<VolatileCell<u32, ReadWrite>>(0x350)
                    .unwrap(),
                phantom: PhantomData,
            }
        }
    }

    pub fn lint1(&self) -> LVTRegister<LINT> {
        unsafe {
            LVTRegister::<LINT> {
                obj: self
                    .mmio
                    .borrow::<VolatileCell<u32, ReadWrite>>(0x360)
                    .unwrap(),
                phantom: PhantomData,
            }
        }
    }

    pub fn error(&self) -> LVTRegister<Error> {
        unsafe {
            LVTRegister::<Error> {
                obj: self
                    .mmio
                    .borrow::<VolatileCell<u32, ReadWrite>>(0x370)
                    .unwrap(),
                phantom: PhantomData,
            }
        }
    }

    pub fn set_spurious_vector(&self, vector: u8) {
        self.write_register(
            Register::SPR,
            *self
                .read_register(Register::SPR)
                .set_bits(0..8, vector as u32),
        );
    }

    pub fn interrupt_command_register(&self) -> icr::InterruptCommandRegister {
        unsafe {
            icr::InterruptCommandRegister::new(
                self.mmio.borrow(Register::ICRL as usize).unwrap(),
                self.mmio.borrow(Register::ICRH as usize).unwrap(),
            )
        }
    }

    pub fn error_status(&self) -> ErrorStatusFlags {
        ErrorStatusFlags::from_bits_truncate(unsafe { self.mmio.read(0x280).unwrap() })
    }

    pub fn read_register(&self, register: Register) -> u32 {
        unsafe { self.mmio.read(register as usize).unwrap() }
    }

    pub fn write_register(&self, register: Register, value: u32) {
        unsafe { self.mmio.write(register as usize, value).unwrap() }
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

    pub fn is_interrupted(&self) -> bool {
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

    pub fn set_vector(&mut self, vector: u8) {
        self.obj
            .write((self.obj.read() & !Self::VECTOR_MASK) | vector as u32);
    }

    #[cfg(debug_assertions)]
    pub fn read_raw(&self) -> u32 {
        self.obj.read()
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
