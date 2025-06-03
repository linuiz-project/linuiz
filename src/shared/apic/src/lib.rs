#![no_std]
#![allow(non_camel_case_types, non_upper_case_globals)]

use bit_field::BitField;
use core::marker::PhantomData;
use msr::IA32_APIC_BASE;

#[repr(u32)]
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
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    OneShot = 0b00,
    Periodic = 0b01,
    TscDeadline = 0b10,
}

impl TryFrom<u32> for TimerMode {
    type Error = u32;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0b00 => Ok(Self::OneShot),
            0b01 => Ok(Self::Periodic),
            0b10 => Ok(Self::TscDeadline),
            value => Err(value),
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
    pub const fn as_divide_value(self) -> u8 {
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
    pub struct ErrorStatusFlags : u32 {
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

#[derive(Debug, Clone, Copy)]
pub struct InterruptCommand {
    apic_id: u32,
    cmd: u32,
}

impl InterruptCommand {
    pub fn new(
        vector: u8,
        apic_id: u32,
        delivery_mode: DeliveryMode,
        is_logical: bool,
        is_assert: bool,
    ) -> Self {
        Self {
            apic_id,
            cmd: *0u32
                .set_bits(0..8, vector.into())
                .set_bits(8..11, delivery_mode as u32)
                .set_bit(11, is_logical)
                .set_bit(14, is_assert),
        }
    }

    #[inline]
    pub fn new_init(apic_id: u32) -> Self {
        Self::new(0, apic_id, DeliveryMode::INIT, false, true)
    }

    #[inline]
    pub fn new_sipi(vector: u8, apic_id: u32) -> Self {
        Self::new(vector, apic_id, DeliveryMode::StartUp, false, true)
    }

    #[inline]
    pub const fn get_id(self) -> u32 {
        self.apic_id
    }

    #[inline]
    pub const fn get_cmd(self) -> u32 {
        self.cmd
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
    ICRL = 0x30,
    ICRH = 0x31,
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
    #[inline]
    pub const fn xapic_offset(self) -> usize {
        (self as usize) * 0x10
    }

    /// Translates this APIC register to its respective x2APIC MSR address.
    #[inline]
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
    pub fn new(map_xapic_fn: Option<impl FnOnce(usize) -> *mut u8>) -> Option<Self> {
        let is_xapic = IA32_APIC_BASE::get_hw_enabled() && !IA32_APIC_BASE::get_is_x2_mode();
        let is_x2apic = IA32_APIC_BASE::get_hw_enabled() && IA32_APIC_BASE::get_is_x2_mode();

        if is_x2apic {
            Some(Self(Type::x2APIC))
        } else if is_xapic {
            let map_xapic_fn = map_xapic_fn.expect("no mapping function provided for xAPIC");
            Some(Self(Type::xAPIC(map_xapic_fn(
                IA32_APIC_BASE::get_base_address().try_into().unwrap(),
            ))))
        } else {
            None
        }
    }

    /// Reads the given register from the local APIC.
    fn read_register(&self, register: Register) -> u32 {
        match self.0 {
            // Safety: Address provided for xAPIC mapping is required to be valid.
            Type::xAPIC(xapic_ptr) => unsafe {
                xapic_ptr
                    .add(register.xapic_offset())
                    .cast::<u32>()
                    .read_volatile()
            },

            // Safety: MSR addresses are known-valid from IA32 SDM.
            Type::x2APIC => unsafe { msr::rdmsr(register.x2apic_msr()).try_into().unwrap() },
        }
    }

    /// ## Safety
    ///
    /// Writing an invalid value to a register is undefined behaviour.
    unsafe fn write_register(&self, register: Register, value: u32) {
        match self.0 {
            Type::xAPIC(xapic_ptr) => xapic_ptr
                .add(register.xapic_offset())
                .cast::<u32>()
                .write_volatile(value),
            Type::x2APIC => msr::wrmsr(register.x2apic_msr(), value.into()),
        }
    }

    /// ## Safety
    ///
    /// Given the amount of external contexts that could potentially rely on the APIC, enabling it
    /// has the oppurtunity to affect those contexts in undefined ways.
    #[inline]
    pub unsafe fn sw_enable(&self) {
        self.write_register(
            Register::SPR,
            *self.read_register(Register::SPR).set_bit(8, true),
        );
    }

    /// ## Safety
    ///
    /// Given the amount of external contexts that could potentially rely on the APIC, disabling it
    /// has the oppurtunity to affect those contexts in undefined ways.
    #[inline]
    pub unsafe fn sw_disable(&self) {
        self.write_register(
            Register::SPR,
            *self.read_register(Register::SPR).set_bit(8, false),
        );
    }

    pub fn get_id(&self) -> u32 {
        self.read_register(Register::ID).get_bits(24..32)
    }

    #[inline]
    pub fn get_version(&self) -> u32 {
        self.read_register(Register::VERSION)
    }

    // TODO maybe unsafe?
    #[inline]
    pub fn end_of_interrupt(&self) {
        unsafe { self.write_register(Register::EOI, 0x0) };
    }

    #[inline]
    pub fn get_error_status(&self) -> ErrorStatusFlags {
        ErrorStatusFlags::from_bits_truncate(self.read_register(Register::ERR))
    }

    /// ## Safety
    ///
    /// An invalid or unexpcted interrupt command could potentially put the core in an unusable state.
    #[inline]
    pub unsafe fn send_int_cmd(&self, interrupt_command: InterruptCommand) {
        self.write_register(Register::ICRL, interrupt_command.get_id());
        self.write_register(Register::ICRH, interrupt_command.get_cmd());
    }

    /// ## Safety
    ///
    /// The timer divisor directly affects the tick rate and interrupt rate of the
    /// internal local timer clock. Thus, changing the divisor has the potential to
    /// cause the same sorts of UB that [`set_timer_initial_count`] can cause.
    #[inline]
    pub unsafe fn set_timer_divisor(&self, divisor: TimerDivisor) {
        self.write_register(Register::TIMER_DIVISOR, divisor.as_divide_value().into());
    }

    /// ## Safety
    ///
    /// Setting the initial count of the timer resets its internal clock. This can lead
    /// to a situation where another context is awaiting a specific clock duration, but
    /// is instead interrupted later than expected.
    #[inline]
    pub unsafe fn set_timer_initial_count(&self, count: u32) {
        self.write_register(Register::TIMER_INT_CNT, count);
    }

    #[inline]
    pub fn get_timer_current_count(&self) -> u32 {
        self.read_register(Register::TIMER_CUR_CNT)
    }

    #[inline]
    pub fn get_timer(&self) -> LocalVector<Timer> {
        LocalVector(self, PhantomData)
    }

    #[inline]
    pub fn get_lint0(&self) -> LocalVector<LINT0> {
        LocalVector(self, PhantomData)
    }

    #[inline]
    pub fn get_lint1(&self) -> LocalVector<LINT1> {
        LocalVector(self, PhantomData)
    }

    #[inline]
    pub fn get_performance(&self) -> LocalVector<Performance> {
        LocalVector(self, PhantomData)
    }

    #[inline]
    pub fn get_thermal_sensor(&self) -> LocalVector<Thermal> {
        LocalVector(self, PhantomData)
    }

    #[inline]
    pub fn get_error(&self) -> LocalVector<Error> {
        LocalVector(self, PhantomData)
    }

    /// Resets the APIC module. The APIC module state is configured as follows:
    ///     - Module is software disabled, then enabled at function end.
    ///     - TPR and TIMER_INT_CNT are zeroed.
    ///     - Timer, Performance, Thermal, and Error local vectors are masked.
    ///     - LINT0 & LINT1 are unmasked and assigned to the `LINT0_VECTOR` (253) and `LINT1_VECTOR` (254), respectively.
    ///     - The spurious register is configured with the `SPURIOUS_VECTOR` (255).
    ///
    /// ## Safety
    ///
    /// The caller must guarantee that software is in a state that is ready to accept the APIC performing a software reset.
    pub unsafe fn software_reset(&self, spr_vector: u8, lint0_vector: u8, lint1_vector: u8) {
        self.sw_disable();

        self.write_register(Register::TPR, 0x0);
        let modified_spr = *self
            .read_register(Register::SPR)
            .set_bits(0..8, spr_vector.into());
        self.write_register(Register::SPR, modified_spr);

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
        self.0
            .read_register(T::REGISTER)
            .get_bit(Self::INTERRUPTED_OFFSET)
    }

    #[inline]
    pub fn get_masked(&self) -> bool {
        self.0
            .read_register(T::REGISTER)
            .get_bit(Self::MASKED_OFFSET)
    }

    /// ## Safety
    ///
    /// Masking an interrupt may result in contexts expecting that interrupt to fire to deadlock.
    #[inline]
    pub unsafe fn set_masked(&self, masked: bool) -> &Self {
        self.0.write_register(
            T::REGISTER,
            *self
                .0
                .read_register(T::REGISTER)
                .set_bit(Self::MASKED_OFFSET, masked),
        );

        self
    }

    #[inline]
    pub fn get_vector(&self) -> Option<u8> {
        match self.0.read_register(T::REGISTER).get_bits(0..8) {
            vector if (0..32).contains(&vector) => None,
            vector => Some(vector as u8),
        }
    }

    /// ## Safety
    ///
    /// Given the vector is an arbitrary >32 `u8`, all contexts must agree on what vectors
    /// correspond to what local interrupts.
    #[inline]
    pub unsafe fn set_vector(&self, vector: u8) -> &Self {
        assert!(vector >= 32, "interrupt vectors 0..32 are reserved");

        self.0.write_register(
            T::REGISTER,
            *self
                .0
                .read_register(T::REGISTER)
                .set_bits(0..8, vector.into()),
        );

        self
    }
}

impl<T: LocalVectorVariant> core::fmt::Debug for LocalVector<'_, T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Local Vector")
            .field(&self.0.read_register(T::REGISTER))
            .finish()
    }
}

impl<T: GenericVectorVariant> LocalVector<'_, T> {
    /// ## Safety
    ///
    /// Setting the incorrect delivery mode may result in interrupts not being received
    /// correctly, or being sent to all cores at once.
    pub unsafe fn set_delivery_mode(&self, mode: DeliveryMode) -> &Self {
        self.0.write_register(
            T::REGISTER,
            *self
                .0
                .read_register(T::REGISTER)
                .set_bits(8..11, mode as u32),
        );

        self
    }
}

impl LocalVector<'_, Timer> {
    #[inline]
    pub fn get_mode(&self) -> TimerMode {
        TimerMode::try_from(
            self.0
                .read_register(<Timer as LocalVectorVariant>::REGISTER)
                .get_bits(17..19),
        )
        .unwrap()
    }

    /// ## Safety
    ///
    /// Setting the mode of the timer may result in undefined behaviour if switching modes while
    /// the APIC is currently active and ticking (or otherwise expecting the timer to behave in
    /// a particular, pre-defined fashion).
    pub unsafe fn set_mode(&self, mode: TimerMode) -> &Self {
        let tsc_dl_support = core::arch::x86_64::__cpuid(0x1).ecx.get_bit(24);

        assert!(
            mode != TimerMode::TscDeadline || tsc_dl_support,
            "TSC deadline is not supported on this CPU."
        );

        self.0.write_register(
            <Timer as LocalVectorVariant>::REGISTER,
            *self
                .0
                .read_register(<Timer as LocalVectorVariant>::REGISTER)
                .set_bits(17..19, mode as u32),
        );

        if tsc_dl_support {
            // IA32 SDM instructs utilizing the `mfence` instruction to ensure all writes to the IA32_TSC_DEADLINE
            // MSR are serialized *after* the APIC timer mode switch (`wrmsr` to `IA32_TSC_DEADLINE` is non-serializing).
            core::arch::asm!("mfence", options(nostack, nomem, preserves_flags));
        }

        self
    }
}
