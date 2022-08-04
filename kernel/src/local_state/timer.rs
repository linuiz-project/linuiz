use alloc::boxed::Box;
use libkernel::structures::apic;

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
    ///         affect software execution, and additionally that the [`crate::interrupts::Vector::LocalTimer`] has
    ///         a proper handler.
    pub unsafe fn new() -> Option<Self> {
        if *apic::xAPIC_SUPPORT || *apic::x2APIC_SUPPORT {
            apic::get_timer().set_mode(apic::TimerMode::OneShot);
            Some(Self(0))
        } else {
            None
        }
    }
}

impl Timer for APICTimer {
    unsafe fn set_frequency(&mut self, set_freq: u64) {
        assert!(
            set_freq <= (u32::MAX as u64),
            "interval frequency cannot be greater than maximum one-shot timer value ({})",
            u32::MAX
        );
        // TODO perhaps check the state of APIC timer LVT? It should be asserted that the below will always work.
        //      Really, in general, the state of the APIC timer should be more carefully controlled. Perhaps this
        //      can be done when the interrupt device is abstracted out into `libkernel`.

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

        let timer_wait = self.0.checked_mul(interval_multiplier).expect("timer interval multiplier overflowed");

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
    pub unsafe fn new() -> Option<Self> {
        if *apic::xAPIC_SUPPORT || *apic::x2APIC_SUPPORT {
            libkernel::cpu::x86_64::FEATURE_INFO.as_ref().filter(|info| info.has_tsc_deadline()).map(|_| {
                apic::get_timer().set_mode(apic::TimerMode::TSC_Deadline);
                Self(0)
            })
        } else {
            None
        }
    }
}

impl Timer for TSCTimer {
    unsafe fn set_frequency(&mut self, set_freq: u64) {
        let freq = libkernel::cpu::x86_64::CPUID
            .get_processor_frequency_info()
            .map(|info| {
                (info.bus_frequency() as u64)
                    / ((info.processor_base_frequency() as u64) * (info.processor_max_frequency() as u64))
            })
            .unwrap_or_else(|| {
                trace!("CPU does not support clock frequency reporting via CPUID.");

                // Wait on the global timer, to ensure we're starting the count
                // on the rising edge of each millisecond.
                crate::clock::busy_wait_msec(1);
                let start_tsc = libkernel::registers::x86_64::TSC::read();
                crate::clock::busy_wait_msec(MS_WINDOW);
                let end_tsc = libkernel::registers::x86_64::TSC::read();

                (end_tsc - start_tsc) * (1000 / MS_WINDOW)
            });

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

        let tsc_wait = self.0.checked_mul(interval_multiplier as u64).expect("timer interval multiplier overflowed");

        libkernel::registers::x86_64::msr::IA32_TSC_DEADLINE::set(libkernel::registers::x86_64::TSC::read() + tsc_wait);
    }
}

/// Gets the best (most precise) local timer available.
///
/// SAFETY: Caller must ensure that the local timer initializing itself
///         will not adversely affect regular control flow.
pub unsafe fn get_best_timer() -> Box<dyn Timer> {
    if let Some(tsc_timer) = TSCTimer::new() {
        Box::new(tsc_timer)
    } else if let Some(apic_timer) = APICTimer::new() {
        Box::new(apic_timer)
    } else {
        panic!("no timers available")
    }
}
