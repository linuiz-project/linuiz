#![no_std]
#![allow(non_camel_case_types, non_upper_case_globals)]
#![feature(exclusive_range_pattern)]

use bit_field::BitField;
use core::marker::PhantomData;
use msr::IA32_APIC_BASE;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    Fixed = 0b000,
    LowPriority = 0b001,
    SMI = 0b010,
    NMI = 0b100,
    INIT = 0b101,
    StartUp = 0b110,
    ExtINT = 0b111,
}

/// Various valid modes for APIC timer to operate in.
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    OneShot = 0b00,
    Periodic = 0b01,
    TscDeadline = 0b10,
}

impl TimerMode {
    #[inline]
    const fn from_u64(value: u64) -> Self {
        match value {
            0b00 => Self::OneShot,
            0b01 => Self::Periodic,
            0b10 => Self::TscDeadline,
            _ => unimplemented!(),
        }
    }
}

/// Divisor for APIC timer to use when not in [`TimerMode::TscDeadline`].
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
    #[repr(transparent)]
    pub struct ErrorStatusFlags : u8 {
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
    pub const fn new(vector: u8, apic_id: u32, delivery_mode: DeliveryMode, is_logical: bool, is_assert: bool) -> Self {
        Self(
            (vector as u64)
                | ((delivery_mode as u64) << 8)
                | ((is_logical as u64) << 11)
                | ((is_assert as u64) << 14)
                | ((apic_id as u64) << 32),
        )
    }

    pub const fn new_init(apic_id: u32) -> Self {
        Self::new(0, apic_id, DeliveryMode::INIT, false, true)
    }

    pub const fn new_sipi(vector: u8, apic_id: u32) -> Self {
        Self::new(vector, apic_id, DeliveryMode::StartUp, false, true)
    }

    pub const fn get_raw(&self) -> u64 {
        self.0
    }
}

/// Various APIC registers, valued as their base register index.
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Register {
    ID = 0x02,
    VERSION = 0x03,
    TPR = 0x08,
    PPR = 0x0A,
    EOI = 0x0B,
    LDR = 0x0C,
    SPR = 0x0F,
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

impl Register {
    /// Translates this APIC register to its respective xAPIC memory offset.
    #[inline(always)]
    pub const fn xapic_offset(self) -> usize {
        (self as usize) * 0x10
    }

    /// Translates this APIC register to its respective x2APIC MSR address.
    #[inline(always)]
    pub const fn x2apic_msr(self) -> u32 {
        x2APIC_BASE_MSR_ADDR + (self as u32)
    }
}

pub const xAPIC_BASE_ADDR: usize = 0xFEE00000;
pub const x2APIC_BASE_MSR_ADDR: u32 = 0x800;

/// Type for representing the mode of the core-local APIC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Type {
    xAPIC(*mut u8),
    x2APIC,
}

pub struct Apic(Type);

impl Apic {
    pub fn new(xapic_map_addr_fn: Option<impl FnOnce(usize) -> *mut u8>) -> Option<Self> {
        let is_xapic = IA32_APIC_BASE::get_hw_enabled() && !IA32_APIC_BASE::get_is_x2_mode();
        let is_x2apic = IA32_APIC_BASE::get_hw_enabled() && IA32_APIC_BASE::get_is_x2_mode();

        match xapic_map_addr_fn {
            Some(xapic_map_addr_fn) if is_xapic => Some(Self(Type::xAPIC(xapic_map_addr_fn(xAPIC_BASE_ADDR)))),
            None if is_x2apic => Some(Self(Type::x2APIC)),
            _ => None,
        }
    }

    /// Reads the given register from the local APIC. Panics if APIC is not properly initialized.
    fn read_register(&self, register: Register) -> u64 {
        match self.0 {
            // ### Safety: Address provided for xAPIC mapping is required to be valid.
            Type::xAPIC(address) => unsafe {
                address.add(register.xapic_offset()).cast::<u32>().read_volatile() as u64
            },

            // ### Safety: MSR addresses are known-valid from IA32 SDM.
            Type::x2APIC => unsafe { msr::rdmsr(register.x2apic_msr()) },
        }
    }

    /// Reads the given register from the local APIC. Panics if APIC is not properly initialized.
    unsafe fn write_register(&self, register: Register, value: u64) {
        match self.0 {
            Type::xAPIC(address) => address.add(register.xapic_offset()).cast::<u32>().write_volatile(value as u32),
            Type::x2APIC => msr::wrmsr(register.x2apic_msr(), value),
        }
    }

    #[inline]
    pub unsafe fn sw_enable(&self) {
        self.write_register(Register::SPR, *self.read_register(Register::SPR).set_bit(8, true));
    }

    #[inline]
    pub unsafe fn sw_disable(&self) {
        self.write_register(Register::SPR, *self.read_register(Register::SPR).set_bit(8, false));
    }

    pub fn get_id(&self) -> u32 {
        self.read_register(Register::ID).get_bits(24..32) as u32
    }

    #[inline]
    pub fn get_version(&self) -> u32 {
        self.read_register(Register::VERSION) as u32
    }

    #[inline]
    pub fn end_of_interrupt(&self) {
        unsafe { self.write_register(Register::EOI, 0x0) };
    }

    #[inline]
    pub fn get_error_status(&self) -> ErrorStatusFlags {
        ErrorStatusFlags::from_bits_truncate(self.read_register(Register::ERR) as u8)
    }

    #[inline]
    pub unsafe fn send_int_cmd(&self, interrupt_command: InterruptCommand) {
        self.write_register(Register::ICR, interrupt_command.get_raw());
    }

    #[inline]
    pub unsafe fn set_timer_divisor(&self, divisor: TimerDivisor) {
        self.write_register(Register::TIMER_DIVISOR, divisor.as_divide_value());
    }

    #[inline]
    pub unsafe fn set_timer_initial_count(&self, count: u32) {
        self.write_register(Register::TIMER_INT_CNT, count as u64);
    }

    #[inline]
    pub fn get_timer_current_count(&self) -> u32 {
        self.read_register(Register::TIMER_CUR_CNT) as u32
    }

    #[inline]
    pub fn get_timer<'a>(&'a self) -> LocalVector<Timer> {
        LocalVector(self, PhantomData)
    }

    #[inline]
    pub fn get_lint0<'a>(&'a self) -> LocalVector<LINT0> {
        LocalVector(self, PhantomData)
    }

    #[inline]
    pub fn get_lint1<'a>(&'a self) -> LocalVector<LINT1> {
        LocalVector(self, PhantomData)
    }

    #[inline]
    pub fn get_performance<'a>(&'a self) -> LocalVector<Performance> {
        LocalVector(self, PhantomData)
    }

    #[inline]
    pub fn get_thermal_sensor<'a>(&'a self) -> LocalVector<Thermal> {
        LocalVector(self, PhantomData)
    }

    #[inline]
    pub fn get_error<'a>(&'a self) -> LocalVector<Error> {
        LocalVector(self, PhantomData)
    }

    /// Resets the APIC module. The APIC module state is configured as follows:
    ///     - Module is software disabled, then enabled at function end.
    ///     - TPR and TIMER_INT_CNT are zeroed.
    ///     - Timer, Performance, Thermal, and Error local vectors are masked.
    ///     - LINT0 & LINT1 are unmasked and assigned to the `LINT0_VECTOR` (253) and `LINT1_VECTOR` (254), respectively.
    ///     - The spurious register is configured with the `SPURIOUS_VECTOR` (255).
    ///
    /// ### Safety
/// 
/// The caller must guarantee that software is in a state that is ready to accept
    ///         the APIC performing a software reset.
    pub unsafe fn software_reset(&self, spr_vector: u8, lint0_vector: u8, lint1_vector: u8) {
        self.sw_disable();

        self.write_register(Register::TPR, 0x0);
        self.write_register(Register::SPR, *self.read_register(Register::SPR).set_bits(0..8, spr_vector as u64));

        self.sw_enable();

        // IA32 SDM specifies that after a software disable, all local vectors
        // are masked, so we need to re-enable the LINTx vectors.
        self.get_lint0().set_masked(false).set_vector(lint0_vector);
        self.get_lint1().set_masked(false).set_vector(lint1_vector);
    }
}

pub trait LocalVectorVariant {
    const REGISTER: Register;
}

pub trait GenericVectorVariant: LocalVectorVariant {}

pub struct Timer;
impl LocalVectorVariant for Timer {
    const REGISTER: Register = Register::LVT_TIMER;
}

pub struct LINT0;
impl LocalVectorVariant for LINT0 {
    const REGISTER: Register = Register::LVT_LINT0;
}
impl GenericVectorVariant for LINT0 {}

pub struct LINT1;
impl LocalVectorVariant for LINT1 {
    const REGISTER: Register = Register::LVT_LINT1;
}
impl GenericVectorVariant for LINT1 {}

pub struct Performance;
impl LocalVectorVariant for Performance {
    const REGISTER: Register = Register::LVT_PERF;
}
impl GenericVectorVariant for Performance {}

pub struct Thermal;
impl LocalVectorVariant for Thermal {
    const REGISTER: Register = Register::LVT_THERMAL;
}
impl GenericVectorVariant for Thermal {}

pub struct Error;
impl LocalVectorVariant for Error {
    const REGISTER: Register = Register::LVT_ERR;
}

#[repr(transparent)]
pub struct LocalVector<'a, T: LocalVectorVariant>(&'a Apic, PhantomData<T>);

impl<T: LocalVectorVariant> LocalVector<'_, T> {
    const INTERRUPTED_OFFSET: usize = 12;
    const MASKED_OFFSET: usize = 16;

    #[inline]
    pub fn get_interrupted(&self) -> bool {
        self.0.read_register(T::REGISTER).get_bit(Self::INTERRUPTED_OFFSET)
    }

    #[inline]
    pub fn get_masked(&self) -> bool {
        self.0.read_register(T::REGISTER).get_bit(Self::MASKED_OFFSET)
    }

    #[inline]
    pub unsafe fn set_masked(&self, masked: bool) -> &Self {
        self.0.write_register(T::REGISTER, *self.0.read_register(T::REGISTER).set_bit(Self::MASKED_OFFSET, masked));

        self
    }

    #[inline]
    pub fn get_vector(&self) -> Option<u8> {
        match self.0.read_register(T::REGISTER).get_bits(0..8) {
            0..32 => None,
            vector => Some(vector as u8),
        }
    }

    #[inline]
    pub unsafe fn set_vector(&self, vector: u8) -> &Self {
        assert!(vector >= 32, "interrupt vectors 0..32 are reserved");

        self.0.write_register(T::REGISTER, *self.0.read_register(T::REGISTER).set_bits(0..8, vector as u64));

        self
    }
}

impl<T: LocalVectorVariant> core::fmt::Debug for LocalVector<'_, T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Local Vector").field(&self.0.read_register(T::REGISTER)).finish()
    }
}

impl<T: GenericVectorVariant> LocalVector<'_, T> {
    #[inline]
    pub unsafe fn set_delivery_mode(&self, mode: DeliveryMode) -> &Self {
        self.0.write_register(T::REGISTER, *self.0.read_register(T::REGISTER).set_bits(8..11, mode as u64));

        self
    }
}

impl LocalVector<'_, Timer> {
    #[inline]
    pub fn get_mode(&self) -> TimerMode {
        TimerMode::from_u64(self.0.read_register(<Timer as LocalVectorVariant>::REGISTER).get_bits(17..19))
    }

    pub unsafe fn set_mode(&self, mode: TimerMode) -> &Self {
        let tsc_dl_support = core::arch::x86_64::__cpuid(0x1).ecx.get_bit(24);

        assert!(mode != TimerMode::TscDeadline || tsc_dl_support, "TSC deadline is not supported on this CPU.");

        self.0.write_register(
            <Timer as LocalVectorVariant>::REGISTER,
            *self.0.read_register(<Timer as LocalVectorVariant>::REGISTER).set_bits(17..19, mode as u64),
        );

        if tsc_dl_support {
            // IA32 SDM instructs utilizing the `mfence` instruction to ensure all writes to the IA32_TSC_DEADLINE
            // MSR are serialized *after* the APIC timer mode switch (`wrmsr` to `IA32_TSC_DEADLINE` is non-serializing).
            core::arch::asm!("mfence", options(nostack, nomem, preserves_flags));
        }

        self
    }
}
