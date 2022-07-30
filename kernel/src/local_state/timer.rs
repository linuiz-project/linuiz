use crate::interrupts::Vector;
use alloc::boxed::Box;
use libkernel::{cpu, structures::apic};

const MS_WINDOW: u64 = 10;

pub trait Timer {
    /// Sets the minimum interval for the timer, in nanoseconds.
    ///
    /// SAFETY: This function likely assumes interrupts are enabled, in the case
    ///         where external timers must be used to determine frequency. Additionally,
    ///         it is assumed the APIC is both hardware and software enabled.
    ///
    ///         If these conditions are not met, it's possible this function will simply
    ///         hang, doing nothing while waiting for timer interrupts that won't occur.
    unsafe fn set_frequency(&mut self, set_freq: u64);

    /// Reloads the timer with the given interval multiplier.
    ///
    /// SAFETY: Caller must ensure reloading the timer will not adversely affect regular
    ///         control flow.
    unsafe fn reload(&mut self, interval_multiplier: u32);
}

/// APIC timer utilizing the built-in APIC clock in one-shot mode.
struct APICTimer(u32);

impl APICTimer {
    /// Creates a new APIC built-in clock timer, in one-shot mode.
    ///
    /// SAFETY: Caller must ensure that reconfiguring the APIC timer mode will not adversely
    ///         affect software execution, and additionally that the [crate::interrupts::Vector::LocalTimer] has
    ///         a proper handler.
    pub unsafe fn new() -> Self {
        apic::get_timer()
            .set_mode(apic::TimerMode::OneShot)
            .set_vector(Vector::LocalTimer as u8);

        Self(0)
    }
}

impl Timer for APICTimer {
    unsafe fn set_frequency(&mut self, set_freq: u64) {
        assert!(
            set_freq <= (u32::MAX as u64),
            "interval frequency cannot be greater than maximum one-shot timer value ({})",
            u32::MAX
        );

        let freq = {
            // Wait on the global timer, to ensure we're starting the count
            // on the rising edge of each millisecond.
            crate::clock::busy_wait_msec(1);
            apic::set_timer_initial_count(u32::MAX);
            crate::clock::busy_wait_msec(MS_WINDOW);

            ((u32::MAX - apic::get_timer_current_count()) as u64) * (1000 / MS_WINDOW)
        };

        trace!("CPU APIC frequency: {} Hz", freq);

        assert!(
            freq >= set_freq,
            "provided frequency is greater than the maximum timer frequency ({} < {})",
            freq,
            set_freq
        );

        self.0 = (freq / set_freq) as u32;
    }

    unsafe fn reload(&mut self, interval_multiplier: u32) {
        assert!(self.0 > 0, "timer frequency has not been configured");

        let timer_wait = self
            .0
            .checked_mul(interval_multiplier)
            .expect("timer interval multiplier overflowed");

        apic::set_timer_initial_count(timer_wait);
    }
}

/// APIC timer utilizing the TSC_DL feature to use the CPU's high-precision timestamp counter.
struct TSCTimer(u64);

impl TSCTimer {
    /// Creates a new TSC-based timer.
    ///
    /// SAFETY: Caller must ensure that reconfiguring the APIC timer mode will not adversely
    ///         affect software execution, and additionally that the [crate::interrupts::Vector::LocalTimer] vector has
    ///         a proper handler.
    pub unsafe fn new() -> Self {
        assert!(
            cpu::has_feature(cpu::Feature::TSC_DL),
            "TSC timer cannot be used without TSC_DL feature"
        );

        apic::get_timer()
            .set_mode(apic::TimerMode::TSC_Deadline)
            .set_vector(Vector::LocalTimer as u8);

        Self(0)
    }
}

impl Timer for TSCTimer {
    unsafe fn set_frequency(&mut self, set_freq: u64) {
        let freq = {
            // Attempt to calculate a concrete frequency via CPUID.
            if let Some(registers) = libkernel::instructions::cpuid::exec(0x15, 0x0)
                .and_then(|result| if result.ebx() > 0 { Some(result) } else { None })
            {
                (registers.ecx() as u64) * ((registers.ebx() as u64) / (registers.eax() as u64))
            }
            // Otherwise, determine frequency with external measurements.
            else {
                trace!("CPU does not support clock frequency reporting via CPUID.");

                // Wait on the global timer, to ensure we're starting the count
                // on the rising edge of each millisecond.
                crate::clock::busy_wait_msec(1);
                let start_tsc = libkernel::registers::TSC::read();
                crate::clock::busy_wait_msec(MS_WINDOW);
                let end_tsc = libkernel::registers::TSC::read();

                (end_tsc - start_tsc) * (1000 / MS_WINDOW)
            }
        };

        trace!("CPU TSC frequency: {} Hz", freq);

        assert!(
            freq >= set_freq,
            "provided frequency is greater than the maximum timer frequency ({} < {})",
            freq,
            set_freq
        );

        self.0 = freq / set_freq;
    }

    unsafe fn reload(&mut self, interval_multiplier: u32) {
        assert!(self.0 > 0, "timer frequency has not been configured");

        let tsc_wait = self
            .0
            .checked_mul(interval_multiplier as u64)
            .expect("timer interval multiplier overflowed");

        libkernel::registers::msr::IA32_TSC_DEADLINE::set(
            libkernel::registers::TSC::read() + tsc_wait,
        );
    }
}

/// Gets the best (most precise) local timer available.
///
/// SAFETY: Caller must ensure that the local timer initializing itself
///         will not adversely affect regular control flow.
pub unsafe fn get_best_timer() -> Box<dyn Timer> {
    if cpu::has_feature(cpu::Feature::TSC_DL) {
        Box::new(TSCTimer::new())
    } else {
        Box::new(APICTimer::new())
    }
}
