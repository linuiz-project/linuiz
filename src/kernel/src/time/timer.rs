#[cfg(target_arch = "x86_64")]
mod timers_impl {
    use alloc::boxed::Box;
    use libarch::x64::structures::apic;

    /// Gets the best (most precise) local timer available.
    ///
    /// SAFETY: This function initializes the APIC timer. The caller must ensure this will not cause
    ///         adverse effects throughout the rest of the program.
    pub unsafe fn configure_new_timer(freq: u16) -> Box<dyn super::Timer> {
        if let Some(tsc_timer) = TSCTimer::new(freq) {
            trace!("Selecting TSC deadline timer: {} Hz", tsc_timer.0 * (freq as u64));
            Box::new(tsc_timer)
        } else if let Some(apic_timer) = APICTimer::new(freq) {
            trace!("Selecting APIC timer: {} Hz", apic_timer.0 * (freq as u32));
            Box::new(apic_timer)
        } else {
            panic!("no timers available! APIC is not supported?")
        }
    }

    /// APIC timer utilizing the built-in APIC clock in one-shot mode.
    struct APICTimer(u32);

    impl APICTimer {
        /// Creates a new APIC built-in clock timer, in one-shot mode.
        ///
        /// SAFETY: Caller must ensure that reconfiguring the APIC timer mode will not adversely
        ///         affect software execution, and additionally that the timer has a proper handler.
        pub unsafe fn new(set_freq: u16) -> Option<Self> {
            if libarch::x64::registers::msr::IA32_APIC_BASE::get_hw_enabled() {
                apic::sw_enable();
                apic::set_timer_divisor(libarch::x64::structures::apic::TimerDivisor::Div1);
                apic::get_timer().set_masked(true).set_mode(apic::TimerMode::OneShot);

                let freq = {
                    let clock = crate::time::clock::get();
                    apic::set_timer_initial_count(u32::MAX);
                    clock.spin_wait_us(super::US_WAIT);
                    let timer_count = apic::get_timer_current_count();

                    (u32::MAX - timer_count) * super::US_FREQ_FACTOR
                };

                // Ensure we reset the APIC timer to avoid any errant interrupts.
                apic::set_timer_initial_count(0);

                Some(Self(freq / (set_freq as u32)))
            } else {
                None
            }
        }
    }

    impl super::Timer for APICTimer {
        unsafe fn set_next_wait(&mut self, interval_multiplier: u16) {
            assert!(self.0 > 0, "timer frequency has not been determined");

            apic::set_timer_initial_count(self.0 * (interval_multiplier as u32));
        }

        unsafe fn enable(&mut self) {
            apic::get_timer().set_masked(false);
        }

        unsafe fn disable(&mut self) {
            apic::get_timer().set_masked(true);
        }
    }

    /// APIC timer utilizing the `TSC_DL` feature to use the CPU's high-precision timestamp counter.
    struct TSCTimer(u64);

    impl TSCTimer {
        /// Creates a new TSC-based timer.
        ///
        /// SAFETY: Caller must ensure that reconfiguring the APIC timer mode will not adversely
        ///         affect software execution, and additionally that the timer has a proper handler.
        pub unsafe fn new(set_freq: u16) -> Option<Self> {
            if libarch::x64::registers::msr::IA32_APIC_BASE::get_hw_enabled()
                && libarch::x64::cpu::cpuid::FEATURE_INFO.has_tsc()
                && libarch::x64::cpu::cpuid::FEATURE_INFO.has_tsc_deadline()
            {
                apic::get_timer().set_mode(apic::TimerMode::TSC_Deadline);

                let freq = libarch::x64::cpu::cpuid::CPUID.get_processor_frequency_info().map_or_else(
                    || {
                        trace!("CPU does not support TSC frequency reporting via CPUID.");

                        apic::sw_enable();
                        apic::get_timer().set_masked(true).set_mode(apic::TimerMode::TSC_Deadline);

                        let clock = crate::time::clock::get();
                        let start_tsc = core::arch::x86_64::_rdtsc();
                        clock.spin_wait_us(super::US_WAIT);
                        let end_tsc = core::arch::x86_64::_rdtsc();

                        (end_tsc - start_tsc) * (super::US_FREQ_FACTOR as u64)
                    },
                    |info| {
                        (info.bus_frequency() as u64)
                            / ((info.processor_base_frequency() as u64) * (info.processor_max_frequency() as u64))
                    },
                );

                Some(Self(freq / (set_freq as u64)))
            } else {
                None
            }
        }
    }

    impl super::Timer for TSCTimer {
        unsafe fn set_next_wait(&mut self, interval_multiplier: u16) {
            assert!(self.0 > 0, "timer frequency has not been determined");

            libarch::x64::registers::msr::IA32_TSC_DEADLINE::set(
                core::arch::x86_64::_rdtsc() + (self.0 * (interval_multiplier as u64)),
            );
        }

        unsafe fn enable(&mut self) {
            apic::get_timer().set_masked(false);
        }

        unsafe fn disable(&mut self) {
            apic::get_timer().set_masked(true);
        }
    }
}

pub use timers_impl::configure_new_timer;

pub(self) const US_PER_SEC: u32 = 1000000;
pub(self) const US_WAIT: u32 = 10000;
pub(self) const US_FREQ_FACTOR: u32 = US_PER_SEC / US_WAIT;

pub trait Timer {
    /// Reloads the timer with the given interval multiplier.
    ///
    /// SAFETY: Caller must ensure reloading the timer will not adversely affect regular
    ///         control flow.
    unsafe fn set_next_wait(&mut self, interval_multiplier: u16);

    /// Enables the timer.
    unsafe fn enable(&mut self);

    /// Disables the timer.
    unsafe fn disable(&mut self);
}
