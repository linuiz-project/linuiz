pub mod icr;

use crate::{
    cell::SyncCell,
    memory::{volatile::VolatileCell, MMIO},
    registers::msr::IA32_APIC_BASE,
    InterruptDeliveryMode, ReadWrite,
};
use bit_field::BitField;
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

pub struct APIC;

lazy_static::lazy_static! {
    static ref APIC_MMIO: SyncCell<MMIO> = SyncCell::new(
        unsafe { MMIO::new_unsafe(IA32_APIC_BASE::get_base_addr().frame_index(), 1) }
    );
}

impl APIC {
    const SPR: usize = 0xF0;
    const EOI: usize = 0xB0;
    const ERRST: usize = 0x280;

    ///
    pub unsafe fn configure_spurious() {
        APIC_MMIO.write(
            Self::SPR,
            *APIC_MMIO
                .read::<u32>(Self::SPR)
                .assume_init()
                .set_bits(0..8, u8::MAX as u32),
        );
    }

    #[inline]
    pub fn read_register(register: Register) -> u32 {
        unsafe { APIC_MMIO.read_unchecked::<u32>(register as usize) }
    }

    #[inline]
    pub fn write_register(register: Register, value: u32) {
        unsafe { APIC_MMIO.write_unchecked(register as usize, value) }
    }

    #[inline]
    pub fn is_hw_enabled() -> bool {
        IA32_APIC_BASE::get_hw_enable()
    }

    #[inline]
    pub unsafe fn hw_enable() {
        IA32_APIC_BASE::set_hw_enable(true);
    }

    #[inline]
    pub unsafe fn hw_disable() {
        IA32_APIC_BASE::set_hw_enable(false);
    }

    #[inline]
    pub unsafe fn sw_enable() {
        APIC_MMIO.write_unchecked(
            Self::SPR,
            *APIC_MMIO.read_unchecked::<u32>(Self::SPR).set_bit(8, true),
        );
    }

    #[inline]
    pub unsafe fn sw_disable() {
        APIC_MMIO.write_unchecked(
            Self::SPR,
            *APIC_MMIO.read_unchecked::<u32>(Self::SPR).set_bit(8, false),
        );
    }

    #[inline]
    pub fn set_eoi_broadcast_suppression(&self, suppress: bool) {
        unsafe {
            APIC_MMIO.write_unchecked(
                Self::SPR,
                *APIC_MMIO
                    .read_unchecked::<u32>(Self::SPR)
                    .set_bit(12, suppress),
            )
        };
    }

    #[inline]
    pub fn end_of_interrupt() {
        unsafe { APIC_MMIO.write_unchecked(Self::EOI, 0) };
    }

    pub fn id() -> u8 {
        Self::read_register(Register::ID).get_bits(24..) as u8
    }

    pub fn version() -> u8 {
        Self::read_register(Register::Version).get_bits(0..8) as u8
    }

    pub fn error_status() -> ErrorStatusFlags {
        ErrorStatusFlags::from_bits_truncate(unsafe { APIC_MMIO.read(Self::ERRST).assume_init() })
    }

    pub fn interrupt_command_register() -> icr::InterruptCommandRegister<'static> {
        unsafe {
            icr::InterruptCommandRegister::new(
                APIC_MMIO.borrow(Register::ICRL as usize),
                APIC_MMIO.borrow(Register::ICRH as usize),
            )
        }
    }

    #[inline]
    pub fn cmci() -> &'static LocalVector<Generic> {
        unsafe { APIC_MMIO.borrow(0x2F0) }
    }

    #[inline]
    pub fn timer() -> &'static LocalVector<Timer> {
        unsafe { APIC_MMIO.borrow(0x320) }
    }

    #[inline]
    pub fn thermal_sensor() -> &'static LocalVector<Generic> {
        unsafe { APIC_MMIO.borrow(0x330) }
    }

    #[inline]
    pub fn performance() -> &'static LocalVector<Generic> {
        unsafe { APIC_MMIO.borrow(0x340) }
    }

    #[inline]
    pub fn lint0() -> &'static LocalVector<LINT> {
        unsafe { APIC_MMIO.borrow(0x350) }
    }

    #[inline]
    pub fn lint1() -> &'static LocalVector<LINT> {
        unsafe { APIC_MMIO.borrow(0x360) }
    }

    #[inline]
    pub fn err() -> &'static LocalVector<Error> {
        unsafe { APIC_MMIO.borrow(0x370) }
    }

    pub unsafe fn reset() {
        Self::sw_disable();
        Self::write_register(Register::DFR, u32::MAX);
        let mut ldr = Self::read_register(Register::LDR);
        ldr &= 0xFFFFFF;
        ldr = (ldr & !0xFF) | ((ldr & 0xFF) | 1);
        Self::performance().set_delivery_mode(InterruptDeliveryMode::NMI);
        Self::write_register(Register::LDR, ldr);
        Self::write_register(Register::TaskPriority, 0);
        Self::write_register(Register::TimerInitialCount, 0);
        Self::cmci().set_masked(true);
        Self::timer().set_masked(true);
        Self::performance().set_masked(true);
        Self::thermal_sensor().set_masked(true);
        Self::err().set_masked(true);
        // Don't mask the LINT0&1 vectors, as they're used for external interrupts (PIC, SMIs, NMIs).
    }
}

pub trait LocalVectorVariant {}

pub enum Timer {}
impl LocalVectorVariant for Timer {}

pub enum Generic {}
impl LocalVectorVariant for Generic {}

pub enum LINT {}
impl LocalVectorVariant for LINT {}

pub enum Error {}
impl LocalVectorVariant for Error {}

#[repr(transparent)]
pub struct LocalVector<T: LocalVectorVariant> {
    vol: VolatileCell<u32, ReadWrite>,
    phantom: PhantomData<T>,
}

impl<T: LocalVectorVariant> crate::memory::volatile::Volatile for LocalVector<T> {}

impl<T: LocalVectorVariant> LocalVector<T> {
    const INTERRUPTED_OFFSET: usize = 12;
    const MASKED_OFFSET: usize = 16;

    #[inline]
    pub fn get_interrupted(&self) -> bool {
        self.vol.read().get_bit(Self::INTERRUPTED_OFFSET)
    }

    #[inline]
    pub fn get_masked(&self) -> bool {
        self.vol.read().get_bit(Self::MASKED_OFFSET)
    }

    #[inline]
    pub fn set_masked(&self, masked: bool) {
        self.vol
            .write(*self.vol.read().set_bit(Self::MASKED_OFFSET, masked));
    }

    #[inline]
    pub fn get_vector(&self) -> Option<u8> {
        match self.vol.read().get_bits(0..8) {
            0..32 => None,
            vector => Some(vector as u8),
        }
    }

    #[inline]
    pub fn set_vector(&self, vector: u8) {
        self.vol
            .write(*self.vol.read().set_bits(0..8, vector as u32));
    }
}

impl<T: LocalVectorVariant> core::fmt::Debug for LocalVector<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Local Vector")
            .field(&format_args!("0b{:b}", self.vol.read()))
            .finish()
    }
}

impl LocalVector<Timer> {
    #[inline]
    pub fn set_mode(&self, mode: TimerMode) {
        assert!(
            mode != TimerMode::TSC_Deadline || crate::cpu::has_feature(crate::cpu::Feature::TSC_DL),
            "TSC deadline is not supported on this CPU."
        );

        self.vol
            .write(*self.vol.read().set_bits(17..19, mode as u32));
    }
}

impl LocalVector<Generic> {
    #[inline]
    pub fn set_delivery_mode(&self, mode: InterruptDeliveryMode) {
        self.vol
            .write(*self.vol.read().set_bits(8..11, mode as u32));
    }
}
