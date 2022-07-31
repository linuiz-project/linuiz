#![allow(non_camel_case_types, non_upper_case_globals)]

mod x;
mod x2;

use crate::InterruptDeliveryMode;
use alloc::boxed::Box;
use bit_field::BitField;
use core::marker::PhantomData;

/// Wrapper trait for easily supporting both xAPIC and x2APIC.
pub(self) trait APIC: Send + Sync {
    /// Reads the given register's value from the local APIC.
    ///
    /// SAFETY: Caller is tasked with ensuring a read to the provided register will not
    ///         result in disruptive control flow.
    unsafe fn read_register(&self, register: Register) -> u64;

    /// Writes the given value to the given register on the local APIC.
    ///
    /// SAFETY: Caller is tasked with ensuring a write to the provided register will not
    ///         result in disruptive control flow.
    unsafe fn write_register(&self, register: Register, value: u64);
}

static LAPIC: crate::cell::SyncOnceCell<Box<dyn APIC>> = unsafe { crate::cell::SyncOnceCell::new() };

/// Error indicating the local APIC has already been initialized.
#[derive(Debug)]
pub struct AlreadyInitError;

/// Initializes the APIC in the most advanced mode possible, and hardware enables the APIC via
/// [crate::registers::msr::IA32_APIC_BASE].
///
/// REMARK: `frame_manager` and `page_manager` must be provided in case there is no hardware
///         support for x2APIC mode. Regardless of whether these arguments are provided,
///         if x2APIC mode is available, it will be enabled and selected.
pub fn init(
    frame_manager: &'static crate::memory::FrameManager,
    page_manager: &'static crate::memory::PageManager,
) -> Result<(), AlreadyInitError> {
    LAPIC
        .set({
            use crate::cpu;

            if cpu::has_feature(cpu::Feature::x2APIC) {
                crate::registers::msr::IA32_APIC_BASE::set(true, true);
                Box::new(x2::x2APIC)
            } else if cpu::has_feature(cpu::Feature::xAPIC) {
                crate::registers::msr::IA32_APIC_BASE::set(true, false);
                Box::new(x::xAPIC(unsafe {
                    crate::memory::MMIO::new(
                        (x::BASE_PTR as usize)..((x::BASE_PTR as usize) + 1),
                        frame_manager,
                        page_manager,
                    )
                    .unwrap()
                }))
            } else {
                panic!("CPU does not support core-local APIC!");
            }
        })
        .map_err(|_| AlreadyInitError)
}

/// Various valid modes for APIC timer to operate in.
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    OneShot = 0b00,
    Periodic = 0b01,
    TSC_Deadline = 0b10,
}

/// Divisor for APIC timer to use when not in [TimerMode::TSC_Deadline].
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Converts the given [TimerDivisor] to its numeric counterpart.
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
    ID = 0x02,
    VERSION = 0x03,
    TPR = 0x08,
    PPR = 0x0A,
    EOI = 0x0B,
    LDR = 0x0C,
    SPURIOUS = 0x0F,
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

pub const LINT0_VECTOR: u8 = 253;
pub const LINT1_VECTOR: u8 = 254;
pub const SPURIOUS_VECTOR: u8 = 255;

unsafe fn read_register(register: Register) -> u64 {
    if let Some(lapic) = LAPIC.get() {
        lapic.read_register(register)
    } else {
        // Fall back to identity-mapped APIC register reading
        x::BASE_PTR.add((register as usize) << 4).read_volatile()
    }
}

unsafe fn write_register(register: Register, value: u64) {
    if let Some(lapic) = LAPIC.get() {
        lapic.write_register(register, value);
    } else {
        // Fall back to identity-mapped APIC register reading.
        x::BASE_PTR.add((register as usize) << 4).write_volatile(value);
    }
}

/*

   APIC FUNCTIONS

*/

#[inline]
pub unsafe fn sw_enable() {
    write_register(Register::SPURIOUS, *read_register(Register::SPURIOUS).set_bit(8, true));
}

#[inline]
pub unsafe fn sw_disable() {
    write_register(Register::SPURIOUS, *read_register(Register::SPURIOUS).set_bit(8, false));
}

#[inline]
pub fn get_id() -> u32 {
    unsafe { read_register(Register::ID) as u32 }
}

#[inline]
pub fn get_version() -> u32 {
    unsafe { read_register(Register::VERSION) as u32 }
}

#[inline]
pub fn end_of_interrupt() {
    unsafe { write_register(Register::EOI, 0x0) };
}

#[inline]
pub fn get_error_status() -> ErrorStatusFlags {
    ErrorStatusFlags::from_bits_truncate(unsafe { read_register(Register::ERR) as u8 })
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
    unsafe { read_register(Register::TIMER_CUR_CNT) as u32 }
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
    write_register(Register::SPURIOUS, *read_register(Register::SPURIOUS).set_bits(0..8, SPURIOUS_VECTOR as u64));

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
        unsafe { read_register(T::REGISTER).get_bit(Self::INTERRUPTED_OFFSET) }
    }

    #[inline]
    pub fn get_masked(&self) -> bool {
        unsafe { read_register(T::REGISTER).get_bit(Self::MASKED_OFFSET) }
    }

    #[inline]
    pub unsafe fn set_masked(&self, masked: bool) -> &Self {
        write_register(T::REGISTER, *read_register(T::REGISTER).set_bit(Self::MASKED_OFFSET, masked));

        self
    }

    #[inline]
    pub fn get_vector(&self) -> Option<u8> {
        match unsafe { read_register(T::REGISTER).get_bits(0..8) } {
            0..32 => None,
            vector => Some(vector as u8),
        }
    }

    #[inline]
    pub unsafe fn set_vector(&self, vector: u8) -> &Self {
        assert!(vector >= 32, "interrupt vectors 0..32 are reserved");

        write_register(T::REGISTER, *read_register(T::REGISTER).set_bits(0..8, vector as u64));

        self
    }
}

impl<T: LocalVectorVariant> core::fmt::Debug for LocalVector<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Local Vector")
            .field(&format_args!("0b{:b}", unsafe { &read_register(T::REGISTER) }))
            .finish()
    }
}

impl<T: GenericVectorVariant> LocalVector<T> {
    #[inline]
    pub unsafe fn set_delivery_mode(&self, mode: InterruptDeliveryMode) -> &Self {
        write_register(T::REGISTER, *read_register(T::REGISTER).set_bits(8..11, mode as u64));

        self
    }
}

impl LocalVector<Timer> {
    pub unsafe fn set_mode(&self, mode: TimerMode) -> &Self {
        use crate::cpu::{has_feature, Feature};

        assert!(
            mode != TimerMode::TSC_Deadline || has_feature(Feature::TSC_DL),
            "TSC deadline is not supported on this CPU."
        );

        write_register(
            <Timer as LocalVectorVariant>::REGISTER,
            *read_register(<Timer as LocalVectorVariant>::REGISTER).set_bits(17..19, mode as u64),
        );

        if has_feature(Feature::TSC_DL) {
            // IA32 SDM instructs utilizing the `mfence` instruction to ensure all writes to the IA32_TSC_DEADLINE
            // MSR are serialized *after* the APIC timer mode switch (`wrmsr` to `IA32_TSC_DEADLINE` is non-serializing).
            crate::instructions::mfence();
        }

        self
    }
}
