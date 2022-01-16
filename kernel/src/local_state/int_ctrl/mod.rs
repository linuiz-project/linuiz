pub(crate) mod handlers;

use core::sync::atomic::AtomicU64;
use libstd::structures::apic::APIC;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterruptVector {
    GlobalTimer = 32,
    LocalTimer = 48,
    CMCI = 49,
    Performance = 50,
    ThermalSensor = 51,
    LINT0 = 52,
    LINT1 = 53,
    Error = 54,
    Storage = 55,
    // APIC spurious interrupt is default mapped to 255.
    Spurious = u8::MAX,
}

pub struct InterruptController {
    apic: APIC,
    counters: [AtomicU64; 256],
}

impl InterruptController {
    const ATOMIC_ZERO: AtomicU64 = AtomicU64::new(0);

    pub fn create() -> Self {
        use libstd::structures::apic::*;

        // Ensure interrupts are enabled.
        libstd::instructions::interrupts::enable();

        trace!("Configuring APIC & APIT.");
        let apic = APIC::from_msr().expect("APIC has already been configured on this core");
        unsafe {
            apic.reset();
            apic.hw_enable();
        }
        apic.write_register(Register::TimerDivisor, TimerDivisor::Div1 as u32);
        apic.timer().set_mode(TimerMode::OneShot);

        let ticks_per_10ms = {
            trace!("Determining APIT frequency.");

            // Wait on the global timer, to ensure we're starting the count on the rising edge of each millisecond.
            crate::clock::global::busy_wait_msec(1);
            // 'Enable' the APIT to begin counting down in `Register::TimerCurrentCount`
            apic.write_register(Register::TimerInitialCount, u32::MAX);
            // Wait for 10ms to get good average tickrate.
            crate::clock::global::busy_wait_msec(10);

            apic.read_register(Register::TimerCurrentCount)
        };

        apic.write_register(
            Register::TimerInitialCount,
            (u32::MAX - ticks_per_10ms) / 10,
        );
        trace!(
            "APIT frequency: {}KHz",
            apic.read_register(Register::TimerInitialCount)
        );

        // Configure timer.
        apic.timer().set_vector(InterruptVector::LocalTimer as u8);
        apic.timer().set_mode(TimerMode::Periodic);
        apic.timer().set_masked(true);
        // Set default vectors.
        apic.cmci().set_vector(InterruptVector::CMCI as u8);
        apic.performance()
            .set_vector(InterruptVector::Performance as u8);
        apic.thermal_sensor()
            .set_vector(InterruptVector::ThermalSensor as u8);
        apic.lint0().set_vector(InterruptVector::LINT0 as u8);
        apic.lint1().set_vector(InterruptVector::LINT1 as u8);
        // Configure error register.
        apic.error().set_vector(InterruptVector::Error as u8);
        apic.error().set_masked(false);

        trace!("Core-local APIC configured and enabled.");

        Self {
            apic,
            counters: [Self::ATOMIC_ZERO; 256],
        }
    }

    #[inline]
    pub fn apic_id(&self) -> u8 {
        self.apic.id()
    }

    #[inline]
    pub unsafe fn sw_enable(&self) {
        self.apic.sw_enable();
    }

    #[inline]
    pub unsafe fn sw_disable(&self) {
        self.apic.sw_disable();
    }

    #[inline]
    pub fn icr(&self) -> libstd::structures::apic::icr::InterruptCommandRegister {
        self.apic.interrupt_command()
    }

    #[inline]
    pub(super) fn end_of_interrupt(&self) {
        self.apic.end_of_interrupt();
    }
}
