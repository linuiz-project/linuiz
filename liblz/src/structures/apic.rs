#![allow(non_camel_case_types)]

use crate::InterruptDeliveryMode;
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

#[repr(u32)]
pub enum Register {
    ID = 0x802,
    VERSION = 0x803,
    TPR = 0x808,
    PPR = 0x80A,
    EOI = 0x80B,
    LDR = 0x80C,
    SPURIOUS = 0x80F,
    ISR0 = 0x810,
    ISR32 = 0x811,
    ISR64 = 0x812,
    ISR96 = 0x813,
    ISR128 = 0x814,
    ISR160 = 0x815,
    ISR192 = 0x816,
    ISR224 = 0x817,
    TMR0 = 0x818,
    TMR32 = 0x819,
    TMR64 = 0x81A,
    TMR96 = 0x81B,
    TMR128 = 0x81C,
    TMR160 = 0x81D,
    TMR192 = 0x81E,
    TMR224 = 0x81F,
    IRR0 = 0x820,
    IRR32 = 0x821,
    IRR64 = 0x822,
    IRR96 = 0x823,
    IRR128 = 0x824,
    IRR160 = 0x825,
    IRR192 = 0x826,
    IRR224 = 0x827,
    ERR = 0x828,
    ICR = 0x830,
    LVT_TIMER = 0x832,
    LVT_THERMAL = 0x833,
    LVT_PERF = 0x834,
    LVT_LINT0 = 0x835,
    LVT_LINT1 = 0x836,
    LVT_ERR = 0x837,
    TIMER_INT_CNT = 0x838,
    TIMER_CUR_CNT = 0x839,
    TIMER_DIVISOR = 0x83E,
    SELF_IPI = 0x83F,
}

pub const LINT0_VECTOR: u8 = 253;
pub const LINT1_VECTOR: u8 = 254;
pub const SPURIOUS_VECTOR: u8 = 255;

#[inline]
fn read_register(register: Register) -> u64 {
    unsafe { crate::registers::msr::rdmsr(register as u32) }
}

#[inline]
unsafe fn write_register(register: Register, value: u64) {
    crate::registers::msr::wrmsr(register as u32, value);
}

/*

   APIC FUNCTIONS

*/

#[inline]
pub unsafe fn sw_enable() {
    write_register(
        Register::SPURIOUS,
        *read_register(Register::SPURIOUS).set_bit(8, true),
    );
}

#[inline]
pub unsafe fn sw_disable() {
    write_register(
        Register::SPURIOUS,
        *read_register(Register::SPURIOUS).set_bit(8, false),
    );
}

#[inline]
pub fn get_id() -> u32 {
    read_register(Register::ID) as u32
}

#[inline]
pub fn get_version() -> u32 {
    read_register(Register::VERSION) as u32
}

#[inline]
pub fn end_of_interrupt() {
    unsafe { write_register(Register::EOI, 0x0) };
}

#[inline]
pub fn get_error_status() -> ErrorStatusFlags {
    ErrorStatusFlags::from_bits_truncate(read_register(Register::ERR) as u8)
}

#[inline]
pub unsafe fn send_int_cmd(int_cmd: InterruptCommand) {
    write_register(Register::ICR, int_cmd.get_raw());
}

#[inline]
pub unsafe fn set_timer_divisor(divisor: TimerDivisor) {
    write_register(Register::TIMER_DIVISOR, divisor.as_divide_value());
}

#[inline]
pub unsafe fn set_timer_initial_count(count: u32) {
    write_register(Register::TIMER_INT_CNT, count as u64);
}

#[inline]
pub fn get_timer_current_count() -> u32 {
    read_register(Register::TIMER_CUR_CNT) as u32
}

/// Resets the APIC module. The APIC module state is configured as follows:
///     - Module is software disabled, then enabled at function end.
///     - TPR and TIMER_INT_CNT are zeroed.
///     - Timer, Performance, Thermal, and Error local vectors are masked.
///     - LINT0 & LINT1 are unmasked and assigned to the `LINT0_VECTOR` (253) and `LINT1_VECTOR` (254), respectively.
///     - The spurious register is configured with the `SPURIOUS_VECTOR` (255).
///
/// SAFETY: The caller must guarantee that software is in a state that is ready to accept
///         the APIC performing a software reset.
pub unsafe fn software_reset() {
    sw_disable();

    write_register(Register::TPR, 0x0);
    write_register(Register::TIMER_INT_CNT, 0x0);
    write_register(
        Register::SPURIOUS,
        *read_register(Register::SPURIOUS).set_bits(0..8, SPURIOUS_VECTOR as u64),
    );

    sw_enable();

    // IA32 SDM specifies that after a software disable, all local vectors
    // are masked, so we need to re-enable the LINT local vectors.
    get_lint0().set_masked(false).set_vector(LINT0_VECTOR);
    get_lint1().set_masked(false).set_vector(LINT1_VECTOR);
}

#[inline]
pub fn get_timer() -> LocalVector<Timer> {
    LocalVector(PhantomData)
}

#[inline]
pub fn get_lint0() -> LocalVector<LINT0> {
    LocalVector(PhantomData)
}

#[inline]
pub fn get_lint1() -> LocalVector<LINT1> {
    LocalVector(PhantomData)
}

#[inline]
pub fn get_performance() -> LocalVector<Performance> {
    LocalVector(PhantomData)
}

#[inline]
pub fn get_thermal_sensor() -> LocalVector<Thermal> {
    LocalVector(PhantomData)
}

#[inline]
pub fn get_error() -> LocalVector<Error> {
    LocalVector(PhantomData)
}

/*

   LOCAL VECTOR TABLE TYPES

*/

pub trait LocalVectorVariant {
    const REGISTER: Register;
}

pub trait GenericVectorVariant: LocalVectorVariant {}

pub enum Timer {}
impl LocalVectorVariant for Timer {
    const REGISTER: Register = Register::LVT_TIMER;
}

pub enum LINT0 {}
impl LocalVectorVariant for LINT0 {
    const REGISTER: Register = Register::LVT_LINT0;
}
impl GenericVectorVariant for LINT0 {}

pub enum LINT1 {}
impl LocalVectorVariant for LINT1 {
    const REGISTER: Register = Register::LVT_LINT1;
}
impl GenericVectorVariant for LINT1 {}

pub enum Performance {}
impl LocalVectorVariant for Performance {
    const REGISTER: Register = Register::LVT_PERF;
}
impl GenericVectorVariant for Performance {}

pub enum Thermal {}
impl LocalVectorVariant for Thermal {
    const REGISTER: Register = Register::LVT_THERMAL;
}
impl GenericVectorVariant for Thermal {}

pub enum Error {}
impl LocalVectorVariant for Error {
    const REGISTER: Register = Register::LVT_ERR;
}

#[repr(transparent)]
pub struct LocalVector<T: LocalVectorVariant>(PhantomData<T>);

impl<T: LocalVectorVariant> crate::memory::volatile::Volatile for LocalVector<T> {}

impl<T: LocalVectorVariant> LocalVector<T> {
    const INTERRUPTED_OFFSET: usize = 12;
    const MASKED_OFFSET: usize = 16;

    #[inline]
    pub fn get_interrupted(&self) -> bool {
        read_register(T::REGISTER).get_bit(Self::INTERRUPTED_OFFSET)
    }

    #[inline]
    pub fn get_masked(&self) -> bool {
        read_register(T::REGISTER).get_bit(Self::MASKED_OFFSET)
    }

    #[inline]
    pub unsafe fn set_masked(&self, masked: bool) -> &Self {
        write_register(
            T::REGISTER,
            *read_register(T::REGISTER).set_bit(Self::MASKED_OFFSET, masked),
        );

        self
    }

    #[inline]
    pub fn get_vector(&self) -> Option<u8> {
        match read_register(T::REGISTER).get_bits(0..8) {
            0..32 => None,
            vector => Some(vector as u8),
        }
    }

    #[inline]
    pub unsafe fn set_vector(&self, vector: u8) -> &Self {
        assert!(vector >= 32, "interrupt vectors 0..32 are reserved");

        write_register(
            T::REGISTER,
            *read_register(T::REGISTER).set_bits(0..8, vector as u64),
        );

        self
    }
}

impl<T: LocalVectorVariant> core::fmt::Debug for LocalVector<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Local Vector")
            .field(&format_args!("0b{:b}", &read_register(T::REGISTER)))
            .finish()
    }
}

impl<T: GenericVectorVariant> LocalVector<T> {
    #[inline]
    pub unsafe fn set_delivery_mode(&self, mode: InterruptDeliveryMode) -> &Self {
        write_register(
            T::REGISTER,
            *read_register(T::REGISTER).set_bits(8..11, mode as u64),
        );

        self
    }
}

impl LocalVector<Timer> {
    pub unsafe fn set_mode(&self, mode: TimerMode) -> &Self {
        assert!(
            mode != TimerMode::TSC_Deadline || crate::cpu::has_feature(crate::cpu::Feature::TSC_DL),
            "TSC deadline is not supported on this CPU."
        );

        write_register(
            <Timer as LocalVectorVariant>::REGISTER,
            *read_register(<Timer as LocalVectorVariant>::REGISTER).set_bits(17..19, mode as u64),
        );

        self
    }
}
