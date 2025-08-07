use crate::{
    arch::x86_64::{
        cpuid::feature_info,
        devices::local_apic::{InterruptDeliveryMode, LocalApic, Register},
    },
    interrupts::Vector,
};
use bit_field::BitField;
use core::marker::PhantomData;
use num_enum::{IntoPrimitive, TryFromPrimitive};

pub trait Kind {
    const REGISTER: super::Register;
}
pub trait Deliverable: Kind {}

pub struct Timer;
impl Kind for Timer {
    const REGISTER: Register = Register::LVT_TIMER;
}

pub struct CMCI;
impl Kind for CMCI {
    const REGISTER: Register = Register::LVT_CMCI;
}
impl Deliverable for CMCI {}

pub struct LINT0;
impl Kind for LINT0 {
    const REGISTER: Register = Register::LVT_LINT0;
}

pub struct LINT1;
impl Kind for LINT1 {
    const REGISTER: Register = Register::LVT_LINT1;
}

pub struct Error;
impl Kind for Error {
    const REGISTER: Register = Register::LVT_ERROR;
}

pub struct PerformanceCounter;
impl Kind for PerformanceCounter {
    const REGISTER: Register = Register::LVT_PERFORMANCE_COUNTER;
}
impl Deliverable for PerformanceCounter {}

pub struct ThermalSensor;
impl Kind for ThermalSensor {
    const REGISTER: Register = Register::LVT_THERMAL_MONITOR;
}
impl Deliverable for ThermalSensor {}

#[derive(Debug, Clone)]
pub struct LocalVector<'a, T> {
    local_apic: &'a LocalApic,
    marker: PhantomData<T>,
}

impl<T: Kind> LocalVector<'_, T> {
    /// Reads the raw LVT entry as a `u32`.
    fn read(&self) -> u32 {
        self.local_apic.read_register(T::REGISTER)
    }

    /// Writes `value` as the raw LVT entry value.
    ///
    /// # Safety
    ///
    /// - `value` must not contain any invalid values.
    /// - `value` must not contain any reserved bits.
    unsafe fn write(&self, value: u32) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe { self.local_apic.write_register(T::REGISTER, value) }
    }

    /// Gets the delivery status of the interrupt.
    ///
    /// - `true` indicates that an interrupt from this source has been delivered
    ///   to the processor core but has not yet been accepted.
    /// - `false` indicates there is currently no activity for this interrupt
    ///   source, or the previous interrupt from this source was delivered to
    ///   the processor core and accepted.
    pub fn get_delivery_status(&self) -> bool {
        self.read().get_bit(12)
    }

    /// Whether the interrupt is masked (ignored upon reception to the APIC).
    ///
    /// Note: When the local APIC handles a performance-monitoring counters
    /// interrupt, it       automatically sets the mask flag in the LVT
    /// performance counter register. This       flag is set to 1 on reset.
    /// It can only be cleared by software.
    pub fn get_masked(&self) -> bool {
        self.read().get_bit(16)
    }

    /// Masks or unmasks the interrupt based on `masked`.
    ///
    /// Note: When the local APIC handles a performance-monitoring counters
    /// interrupt, it       automatically sets the mask flag in the LVT
    /// performance counter register. This       flag is set to 1 on reset.
    /// It can only be cleared by software.
    pub fn set_masked(&self, masked: bool) -> &Self {
        // Safety: The masked bit is valid and non-reserved.
        unsafe {
            self.write(*self.read().set_bit(16, masked));
        }

        self
    }

    /// Gets the interrupt vector number.
    pub fn get_vector(&self) -> Vector {
        let vector = self.read().get_bits(0..8);

        debug_assert!(vector > 15, "interrupts vectors 0..=15 are reserved");

        Vector::from(u8::try_from(vector).unwrap())
    }

    /// Sets the interrupt vector number.
    pub fn set_vector(&self, vector: Vector) -> &Self {
        let vector = u8::from(vector);

        debug_assert!(vector > 15, "interrupts vectors 0..=15 are reserved");

        // Safety: The vector bits are valid and non-reserved.
        unsafe {
            self.write(*self.read().set_bits(0..8, u32::from(vector)));
        }

        self
    }
}

impl<T: Deliverable> LocalVector<'_, T> {
    /// Specifies the type of interrupt to be sent to the processor. Some
    /// delivery modes will only operate as intended when used in
    /// conjunction with a specific trigger mode.
    pub fn set_delivery_mode(&self, mode: InterruptDeliveryMode) -> &Self {
        // Safety: The delivery mode bits are valid and non-reserved.
        unsafe {
            self.write(*self.read().set_bits(8..11, u32::from(mode)));
        }

        self
    }
}

/// Various valid modes for APIC timer to operate.
#[repr(u32)]
#[derive(Debug, IntoPrimitive, TryFromPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    /// Timer will operate in a one-shot mode using a count-down value.
    OneShot = 0b00,

    /// Timer will operate in a periodic mode by reloading a count-down value.
    Periodic = 0b01,

    /// Uses the `IA32_TSC_DEADLINE` model-specific register as a deadline
    /// value, which will trigger when the hardware thread's timestamp
    /// counter reaches or passes the deadline.
    TscDeadline = 0b10,
}

impl LocalVector<'_, Timer> {
    /// Gets the mode that the timer is currently operating in.
    pub fn get_mode(&self) -> TimerMode {
        TimerMode::try_from(self.read().get_bits(17..19)).unwrap()
    }

    /// Sets the mode for the timer to operate in.
    pub fn set_mode(&self, mode: TimerMode) -> &Self {
        if mode == TimerMode::TscDeadline
            && !feature_info().is_some_and(raw_cpuid::FeatureInfo::has_tsc_deadline)
        {
            panic!("timestamp counter deadline timer mode not supported");
        }

        // Safety: The timer mode bits are valid and non-reserved.
        unsafe {
            self.write(*self.read().set_bits(17..19, u32::from(mode)));
        }

        self
    }
}

impl LocalApic {
    pub fn lvt_lint0(&self) -> LocalVector<LINT0> {
        LocalVector {
            local_apic: self,
            marker: PhantomData,
        }
    }

    pub fn lvt_lint1(&self) -> LocalVector<LINT1> {
        LocalVector {
            local_apic: self,
            marker: PhantomData,
        }
    }

    pub fn lvt_error(&self) -> LocalVector<Error> {
        LocalVector {
            local_apic: self,
            marker: PhantomData,
        }
    }

    pub fn lvt_timer(&self) -> LocalVector<Timer> {
        LocalVector {
            local_apic: self,
            marker: PhantomData,
        }
    }

    pub fn lvt_performance_counter(&self) -> Option<LocalVector<PerformanceCounter>> {
        // If max LVT is 4, then 5 registers are supported, which includes
        // the performance counter register.
        (self.max_lvt_entry() >= 4).then_some(LocalVector {
            local_apic: self,
            marker: PhantomData,
        })
    }

    pub fn lvt_thermal_monitor(&self) -> Option<LocalVector<ThermalSensor>> {
        // If max LVT is 5, then 6 registers are supported, which includes
        // thermal monitor register.
        (self.max_lvt_entry() >= 5).then_some(LocalVector {
            local_apic: self,
            marker: PhantomData,
        })
    }

    pub fn lvt_cmci(&self) -> Option<LocalVector<CMCI>> {
        // If max LVT is 6, then 7 registers are supported, which includes
        // the CMCI register.
        (self.max_lvt_entry() >= 6).then_some(LocalVector {
            local_apic: self,
            marker: PhantomData,
        })
    }
}
