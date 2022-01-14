use core::sync::atomic::AtomicU64;
use libstd::structures::apic::APIC;

mod handlers;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterruptVector {
    GlobalTimer = 32,
    LocalTimer = 48,
    CMCI,
    Performance,
    ThermalSensor,
    LINT0,
    LINT1,
    Error,
    Storage,
    // APIC spurious interrupt is default mapped to 255.
    Spurious = u8::MAX,
}

pub struct InterruptController {
    apic: APIC,
    counters: [AtomicU64; 256],
}

pub unsafe fn configure_default_idt_handlers() {}

impl InterruptController {
    const ATOMIC_ZERO: AtomicU64 = AtomicU64::new(0);

    pub fn create() -> Self {
        use libstd::structures::{apic::*, idt};

        extern "x86-interrupt" fn apit_counter_deplete(_: idt::InterruptStackFrame) {
            panic!("APIT has fired an interrupt during initialization (NOTE: APIT should not have enough time to fully deplete its counter register).");
        }

        debug!("Configuring APIC & APIT.");
        let apic = APIC::from_msr().expect("APIC has already been configured on this core");
        unsafe {
            apic.hw_enable();
            apic.reset();
        }
        apic.write_register(Register::TimerDivisor, TimerDivisor::Div1 as u32);
        apic.timer().set_vector(InterruptVector::LocalTimer as u8);
        apic.timer().set_mode(TimerMode::OneShot);

        idt::set_interrupt_handler(InterruptVector::Spurious as u8, handlers::spurious_handler);
        idt::set_interrupt_handler(InterruptVector::Error as u8, handlers::error_handler);
        libstd::instructions::interrupts::enable();

        unsafe {
            debug!("Determining APIT frequency.");

            // Configure PIT & APIT interrupts.
            idt::set_interrupt_handler(InterruptVector::LocalTimer as u8, apit_counter_deplete);

            // Make sure the global timer is already configured.
            crate::clock::global::sleep_msec(1);
            // 'Enable' the APIT to begin counting down in `Register::TimerCurrentCount`
            apic.write_register(Register::TimerInitialCount, u32::MAX);
            // Wait for 10ms to get good average tickrate.
            crate::clock::global::sleep_msec(10);

            // Set final handler for APIT.
            idt::set_interrupt_handler(InterruptVector::LocalTimer as u8, handlers::apit_handler);
        }

        apic.write_register(
            Register::TimerInitialCount,
            (u32::MAX - apic.read_register(Register::TimerCurrentCount)) / 10,
        );
        debug!(
            "APIT frequency: {}KHz",
            apic.read_register(Register::TimerInitialCount)
        );

        // Configure timer.
        apic.timer().set_mode(TimerMode::Periodic);
        apic.timer().set_masked(false);
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

        debug!("Core-local APIC configured and enabled.");

        Self {
            apic,
            counters: [Self::ATOMIC_ZERO; 256],
        }
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
    pub fn interrupt_command(&self) -> libstd::structures::apic::icr::InterruptCommandRegister {
        self.apic.interrupt_command()
    }

    #[inline]
    pub(super) fn end_of_interrupt(&self) {
        self.apic.end_of_interrupt();
    }
}
