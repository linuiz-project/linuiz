use core::fmt::{Debug, Formatter, Result};

use bit_field::BitField;
use lib::{
    memory::{
        volatile::{Volatile, VolatileCell},
        MMIO,
    },
    ReadWrite,
};

#[repr(transparent)]
pub struct Capabilities(VolatileCell<u64, ReadWrite>);

impl Volatile for Capabilities {}
impl Capabilities {
    pub fn rev_id(&self) -> u8 {
        self.0.read().get_bits(0..8) as u8
    }

    pub fn timer_count(&self) -> u8 {
        (self.0.read().get_bits(8..13) + 1) as u8
    }

    pub fn ia64_capable(&self) -> bool {
        self.0.read().get_bit(13)
    }

    pub fn legacy_irqs(&self) -> bool {
        self.0.read().get_bit(15)
    }

    pub fn vendor_id(&self) -> u16 {
        self.0.read().get_bits(16..32) as u16
    }

    pub fn clock_period(&self) -> u32 {
        self.0.read().get_bits(32..64) as u32
    }
}

impl Debug for Capabilities {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        formatter
            .debug_struct("Capabilities")
            .field("Revision ID", &self.rev_id())
            .field("Timer Count", &self.timer_count())
            .field("IA64 Capable", &self.ia64_capable())
            .field("Legacy IRQs", &self.legacy_irqs())
            .field("Vendor ID", &self.vendor_id())
            .field("Clock Period", &self.clock_period())
            .finish()
    }
}

#[repr(transparent)]
pub struct Config(VolatileCell<u64, ReadWrite>);

impl Volatile for Config {}
impl Config {
    pub fn set_enable(&mut self, enable: bool) {
        // TODO generalize this stuff. It's repeated a lot.
        self.0.write(*self.0.read().set_bit(0, enable));
    }

    pub fn set_legacy_irqs(&mut self, enable: bool) {
        self.0.write(*self.0.read().set_bit(1, enable));
    }
}

impl Debug for Config {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        formatter
            .debug_struct("Config")
            .field("Enabled", &self.0.read().get_bit(0))
            .field("Legacy IRQs Enabled", &self.0.read().get_bit(1))
            .finish()
    }
}

#[repr(transparent)]
pub struct InterruptStatus(VolatileCell<u64, ReadWrite>);

impl Volatile for InterruptStatus {}
impl InterruptStatus {
    pub fn get_statuses(&self) -> u32 {
        self.0.read().get_bits(0..32) as u32
    }
}

impl Debug for InterruptStatus {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        formatter
            .debug_tuple("InterruptStatus")
            .field(&format_args!("0b{:b}", &self.get_statuses()))
            .finish()
    }
}

#[repr(transparent)]
pub struct MainCounter(VolatileCell<u64, ReadWrite>);

impl Volatile for MainCounter {}
impl MainCounter {
    pub fn get(&self) -> u64 {
        self.0.read()
    }

    pub fn set(&mut self, value: u64) {
        self.0.write(value);
    }
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerTriggerType {
    EdgeTriggered = 0,
    LevelTriggered = 1,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatingMode {
    NonPeriodic = 0,
    Periodic = 1,
}

#[repr(transparent)]
pub struct TimerData(VolatileCell<u64, ReadWrite>);

impl TimerData {
    pub fn trigger_type(&self) -> TimerTriggerType {
        if self.0.read().get_bit(1) {
            TimerTriggerType::LevelTriggered
        } else {
            TimerTriggerType::EdgeTriggered
        }
    }

    pub fn set_enable(&self, enable: bool) {
        self.0.write(*self.0.read().set_bit(2, enable));
    }

    pub fn get_op_mode(&self) -> OperatingMode {
        if self.0.read().get_bit(3) {
            OperatingMode::NonPeriodic
        } else {
            OperatingMode::Periodic
        }
    }

    pub fn set_op_mode(&self, op_mode: OperatingMode) {
        let bit = match op_mode {
            OperatingMode::NonPeriodic => false,
            OperatingMode::Periodic => {
                assert!(
                    self.period_capable(),
                    "Periodic mode is not supported by this timer."
                );

                true
            }
        };

        self.0.write(*self.0.read().set_bit(3, bit));
    }

    pub fn period_capable(&self) -> bool {
        self.0.read().get_bit(4)
    }

    pub fn is_64_bit(&self) -> bool {
        self.0.read().get_bit(5)
    }

    pub fn set_mut_accumulator(&self) {
        assert!(
            self.get_op_mode() == OperatingMode::Periodic,
            "Accumulator cannot be modified in non-periodic mode."
        );

        self.0.write(*self.0.read().set_bit(6, true));
    }

    pub fn get_force_32_bit(&self) -> bool {
        self.0.read().get_bit(8)
    }

    pub fn set_force_32_bit(&self, enable32bit: bool) {
        self.0.write(*self.0.read().set_bit(8, enable32bit));
    }

    pub fn get_ioapic_route(&self) -> Option<u64> {
        match self.0.read().get_bits(9..14) {
            0 => None,
            route => Some(route),
        }
    }

    pub fn set_ioapic_route(&self, value: Option<u64>) {
        if let Some(value) = value {
            assert!(
                !self.get_fsb_enable(),
                "FSB and IOAPIC routing cannot be enabled at the same time."
            );
        }

        let value = value.unwrap_or(0);
        self.0.write(*self.0.read().set_bits(9..14, value));
        assert_eq!(
            self.0.read().get_bits(9..14),
            value,
            "Provided IOAPIC route is not supported."
        );
    }

    pub fn get_fsb_enable(&self) -> bool {
        self.0.read().get_bit(14)
    }

    pub fn set_fsb_enable(&self, enable: bool) {
        assert!(
            self.get_fsb_capable(),
            "Timer does not support front-side bus interrupt delivery."
        );

        assert!(
            self.get_ioapic_route().is_none(),
            "FSB and IOAPIC routing cannot be enabled at the same time."
        );

        self.0.write(*self.0.read().set_bit(14, enable));
    }

    pub fn get_fsb_capable(&self) -> bool {
        self.0.read().get_bit(15)
    }

    pub fn int_routes_supported(&self) -> u32 {
        self.0.read().get_bits(32..64) as u32
    }
}

pub enum TimerComparator<'c> {
    Dword(&'c VolatileCell<u32, ReadWrite>),
    Qword(&'c VolatileCell<u64, ReadWrite>),
}

#[repr(C)]
pub struct TimerRegister {
    data: TimerData,
    comparator: VolatileCell<u64, ReadWrite>,
    fsb: u64,
}

impl TimerRegister {
    pub fn data(&self) -> &TimerData {
        &self.data
    }

    pub fn comparator(&self) -> TimerComparator {
        if self.data().is_64_bit() && !self.data().get_force_32_bit() {
            TimerComparator::Qword(&self.comparator)
        } else {
            TimerComparator::Dword(unsafe { &*((&raw const self.comparator) as *const _) })
        }
    }
}

pub struct HighPerfTimer {
    min_tick: u16,
    freq: u64,
    mmio: MMIO,
}

impl HighPerfTimer {
    const CAPABILITIES: usize = 0x0;
    const CONFIG: usize = 0x10;
    const INTERRUPT_STATUS: usize = 0x20;
    const MAIN_COUNTER: usize = 0xF0;
    const TIMERS_BASE: usize = 0x100;

    // pub unsafe fn from_acpi_tables() -> Option<Self> {
    //     use lib::acpi::rdsp::xsdt::{hpet::HPET, XSDT};

    //     XSDT.find_sub_table::<HPET>().ok().map(|hpet| {
    //         let mut hpet_driver = Self {
    //             min_tick: hpet.min_tick(),
    //             mmio: MMIO::new(
    //                 {
    //                     let address = hpet.address_data().address;
    //                     address.frame_index()
    //                 },
    //                 1,
    //             )
    //             .unwrap(),
    //             freq: 0,
    //         };

    //         hpet_driver.freq = u32::pow(10, 15) / hpet_driver.capabilities().clock_period();
    //     })
    // }

    pub fn capabilities(&self) -> &Capabilities {
        unsafe { self.mmio.borrow(Self::CAPABILITIES) }
    }

    pub fn config(&self) -> &Config {
        unsafe { self.mmio.borrow(Self::CONFIG) }
    }

    pub fn interrupt_status(&self) -> &InterruptStatus {
        unsafe { self.mmio.borrow(Self::INTERRUPT_STATUS) }
    }

    pub fn main_counter(&self) -> &MainCounter {
        unsafe { self.mmio.borrow(Self::MAIN_COUNTER) }
    }
}

impl Debug for HighPerfTimer {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        formatter
            .debug_struct("HPET")
            .field("Minimum Tick", &self.min_tick)
            .field("Capabilities", &self.capabilities())
            .field("Config", &self.config())
            .field("Interrupt Status", &self.interrupt_status())
            .finish()
    }
}
