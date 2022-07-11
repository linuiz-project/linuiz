#![allow(non_camel_case_types)]

mod x2apic;
mod xapic;

pub use x2apic::*;
pub use xapic::*;

use crate::{registers::msr::IA32_APIC_BASE, InterruptDeliveryMode};
use bit_field::BitField;
use core::marker::PhantomData;

#[repr(u64)]
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
    pub const fn as_divide_value(self) -> u64 {
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

pub struct InterruptCommand(u64);

impl InterruptCommand {
    pub const fn new(
        vector: u8,
        apic_id: u32,
        delivery_mode: InterruptDeliveryMode,
        is_logical: bool,
        is_assert: bool,
    ) -> Self {
        Self(
            (vector as u64)
                | ((delivery_mode as u64) << 8)
                | ((is_logical as u64) << 11)
                | ((is_assert as u64) << 14)
                | ((apic_id as u64) << 32),
        )
    }

    pub const fn new_init(apic_id: u32) -> Self {
        Self::new(0, apic_id, crate::InterruptDeliveryMode::INIT, false, true)
    }

    pub const fn new_sipi(vector: u8, apic_id: u32) -> Self {
        Self::new(vector, apic_id, InterruptDeliveryMode::StartUp, false, true)
    }

    pub const fn get_raw(&self) -> u64 {
        self.0
    }
}

lazy_static::lazy_static! {
    static ref VERSION: Version = {
        if crate::cpu::has_feature(crate::cpu::Feature::X2APIC) {
            IA32_APIC_BASE::set_hw_enable(true);
            IA32_APIC_BASE::set_x2_mode(true);

            Version::x2APIC
        } else {
            IA32_APIC_BASE::set_hw_enable(true);

            Version::xAPIC
        }
    };
}

#[derive(Debug, PartialEq, Eq)]
enum Version {
    xAPIC,
    x2APIC,
}

#[repr(usize)]
pub enum Offset {
    ID = 0x2,
    VERSION = 0x3,
    TPR = 0x8,
    PPR = 0xA,
    EOI = 0xB,
    /// Logical Destination Register
    LDR = 0xD,
    /// Destinaton Format Register
    /// REMARK: GPF when writing in x2APIC mode.
    DFR = 0xE,
    SPURIOUS = 0xF,
    ISR0 = 0x10,
    ISR32 = 0x11,
    ISR64 = 0x12,
    ISR96 = 0x13,
    ISR128 = 0x14,
    ISR160 = 0x15,
    ISR192 = 0x16,
    ISR224 = 0x17,
    TMR0 = 0x18,
    TMR32 = 0x19,
    TMR64 = 0x1A,
    TMR96 = 0x1B,
    TMR128 = 0x1C,
    TMR160 = 0x1D,
    TMR192 = 0x1E,
    TMR224 = 0x1F,
    IRR0 = 0x20,
    IRR32 = 0x21,
    IRR64 = 0x22,
    IRR96 = 0x23,
    IRR128 = 0x24,
    IRR160 = 0x25,
    IRR192 = 0x26,
    IRR224 = 0x27,
    ERR = 0x28,
    ICR = 0x30,
    LVT_TIMER = 0x32,
    LVT_THERMAL = 0x33,
    LVT_PERF = 0x34,
    LVT_LINT0 = 0x35,
    LVT_LINT1 = 0x36,
    LVT_ERR = 0x37,
    TIMER_INT_CNT = 0x38,
    TIMER_CUR_CNT = 0x39,
    TIMER_DIVISOR = 0x3E,
    SELF_IPI = 0x3F,
}

pub(self) trait APIC {
    fn read_offset(offset: Offset) -> u64;
    unsafe fn write_offset(offset: Offset, value: u64);
    unsafe fn send_int_cmd(int_cmd: InterruptCommand);
}

macro_rules! with_apic {
    (|$apic:ident| $code:block) => {
        match *VERSION {
            Version::xAPIC => {
                type $apic = xAPIC;
                $code
            }
            Version::x2APIC => {
                type $apic = x2APIC;
                $code
            }
        }
    };
}

macro_rules! read_offset {
    ($offset:expr) => {
        match *VERSION {
            Version::xAPIC => xAPIC::read_offset($offset),
            Version::x2APIC => x2APIC::read_offset($offset),
        }
    };
}

macro_rules! write_offset {
    ($offset:expr, $value:expr) => {
        match *VERSION {
            Version::xAPIC => xAPIC::write_offset($offset, $value),
            Version::x2APIC => x2APIC::write_offset($offset, $value),
        }
    };
}

pub unsafe fn sw_enable() {
    write_offset!(
        Offset::SPURIOUS,
        *read_offset!(Offset::SPURIOUS).set_bit(8, true)
    );
}

pub unsafe fn sw_disable() {
    write_offset!(
        Offset::SPURIOUS,
        *read_offset!(Offset::SPURIOUS).set_bit(8, false)
    );
}

pub fn get_id() -> u32 {
    read_offset!(Offset::ID) as u32
}

pub fn get_version() -> u32 {
    read_offset!(Offset::VERSION) as u32
}

pub fn end_of_interrupt() {
    unsafe { write_offset!(Offset::EOI, 0x0) };
}

pub fn get_error_status() -> ErrorStatusFlags {
    ErrorStatusFlags::from_bits_truncate(read_offset!(Offset::ERR) as u8)
}

pub unsafe fn send_int_cmd(int_cmd: InterruptCommand) {
    with_apic!(|APIC| { APIC::send_int_cmd(int_cmd) })
}

pub unsafe fn configure_spurious(vector: u8) {
    assert!(vector >= 32, "interrupt vectors 0..32 are reserved");

    write_offset!(
        Offset::SPURIOUS,
        *read_offset!(Offset::SPURIOUS).set_bits(0..8, vector as u64)
    );
}

pub unsafe fn set_timer_divisor(divisor: TimerDivisor) {
    write_offset!(Offset::TIMER_DIVISOR, divisor.as_divide_value());
}

pub unsafe fn set_timer_initial_count(count: u32) {
    write_offset!(Offset::TIMER_INT_CNT, count as u64);
}

pub fn get_timer_current_count() -> u32 {
    read_offset!(Offset::TIMER_CUR_CNT) as u32
}

pub unsafe fn reset() {
    sw_disable();
    if *VERSION != Version::x2APIC {
        write_offset!(Offset::DFR, u32::MAX as u64);
    }
    let mut ldr = read_offset!(Offset::LDR);
    ldr &= 0xFFFFFF;
    ldr = (ldr & !0xFF) | ((ldr & 0xFF) | 1);
    write_offset!(Offset::LDR, ldr);
    write_offset!(Offset::TPR, 0x0);
    write_offset!(Offset::TIMER_INT_CNT, 0x0);
    get_timer().set_masked(true);
    get_performance().set_masked(true);
    get_performance().set_delivery_mode(InterruptDeliveryMode::NMI);
    get_thermal_sensor().set_masked(true);
    get_error().set_masked(true);
    // Don't mask the LINT0&1 vectors, as they're used for external interrupts (PIC, SMIs, NMIs).
}

pub fn get_timer() -> LocalVector<Timer> {
    LocalVector(PhantomData)
}

pub fn get_lint0() -> LocalVector<LINT0> {
    LocalVector(PhantomData)
}

pub fn get_lint1() -> LocalVector<LINT1> {
    LocalVector(PhantomData)
}

pub fn get_performance() -> LocalVector<Performance> {
    LocalVector(PhantomData)
}

pub fn get_thermal_sensor() -> LocalVector<Thermal> {
    LocalVector(PhantomData)
}

pub fn get_error() -> LocalVector<Error> {
    LocalVector(PhantomData)
}

/*

   LOCAL VECTOR TABLE TYPES

*/

pub trait LocalVectorVariant {
    const OFFSET: Offset;
}

pub trait GenericVectorVariant: LocalVectorVariant {}

pub enum Timer {}
impl LocalVectorVariant for Timer {
    const OFFSET: Offset = Offset::LVT_TIMER;
}

pub enum LINT0 {}
impl LocalVectorVariant for LINT0 {
    const OFFSET: Offset = Offset::LVT_LINT0;
}
impl GenericVectorVariant for LINT0 {}

pub enum LINT1 {}
impl LocalVectorVariant for LINT1 {
    const OFFSET: Offset = Offset::LVT_LINT1;
}
impl GenericVectorVariant for LINT1 {}

pub enum Performance {}
impl LocalVectorVariant for Performance {
    const OFFSET: Offset = Offset::LVT_PERF;
}
impl GenericVectorVariant for Performance {}

pub enum Thermal {}
impl LocalVectorVariant for Thermal {
    const OFFSET: Offset = Offset::LVT_THERMAL;
}
impl GenericVectorVariant for Thermal {}

pub enum Error {}
impl LocalVectorVariant for Error {
    const OFFSET: Offset = Offset::LVT_ERR;
}

#[repr(transparent)]
pub struct LocalVector<T: LocalVectorVariant>(PhantomData<T>);

impl<T: LocalVectorVariant> crate::memory::volatile::Volatile for LocalVector<T> {}

impl<T: LocalVectorVariant> LocalVector<T> {
    const INTERRUPTED_OFFSET: usize = 12;
    const MASKED_OFFSET: usize = 16;

    #[inline]
    pub fn get_interrupted(&self) -> bool {
        read_offset!(T::OFFSET).get_bit(Self::INTERRUPTED_OFFSET)
    }

    #[inline]
    pub fn get_masked(&self) -> bool {
        read_offset!(T::OFFSET).get_bit(Self::MASKED_OFFSET)
    }

    #[inline]
    pub unsafe fn set_masked(&self, masked: bool) {
        write_offset!(
            T::OFFSET,
            *read_offset!(T::OFFSET).set_bit(Self::MASKED_OFFSET, masked)
        );
    }

    #[inline]
    pub fn get_vector(&self) -> Option<u8> {
        match read_offset!(T::OFFSET).get_bits(0..8) {
            0..32 => None,
            vector => Some(vector as u8),
        }
    }

    #[inline]
    pub unsafe fn set_vector(&self, vector: u8) {
        assert!(vector >= 32, "interrupt vectors 0..32 are reserved");

        write_offset!(
            T::OFFSET,
            *read_offset!(T::OFFSET).set_bits(0..8, vector as u64)
        );
    }
}

impl<T: LocalVectorVariant> core::fmt::Debug for LocalVector<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Local Vector")
            .field(&format_args!("0b{:b}", &read_offset!(T::OFFSET)))
            .finish()
    }
}

impl<T: GenericVectorVariant> LocalVector<T> {
    #[inline]
    pub unsafe fn set_delivery_mode(&self, mode: InterruptDeliveryMode) {
        write_offset!(
            T::OFFSET,
            *read_offset!(T::OFFSET).set_bits(8..11, mode as u64)
        );
    }
}

impl LocalVector<Timer> {
    #[inline]
    pub unsafe fn set_mode(&self, mode: TimerMode) {
        assert!(
            mode != TimerMode::TSC_Deadline || crate::cpu::has_feature(crate::cpu::Feature::TSC_DL),
            "TSC deadline is not supported on this CPU."
        );

        write_offset!(
            <Timer as LocalVectorVariant>::OFFSET,
            *read_offset!(<Timer as LocalVectorVariant>::OFFSET).set_bits(17..19, mode as u64)
        );
    }
}
