use crate::interrupts::apic;
use crate::time::clock;
use alloc::boxed::Box;

/// Gets the best (most precise) local timer available.
///
/// SAFETY: This function initializes the APIC timer. The caller must ensure this will not cause
///         adverse effects throughout the rest of the program.
pub unsafe fn configure_new_timer(freq: u16) -> Box<dyn Timer> {
    if let Some(tsc_timer) = TSCTimer::new(freq) {
        Box::new(tsc_timer)
    } else if let Some(apic_timer) = APICTimer::new(freq) {
        Box::new(apic_timer)
    } else {
        panic!("no timers available! APIC is not supported?")
    }
}

const US_PER_SEC: u64 = 1000000;
const US_WAIT: u64 = 10000;
const US_FREQ_FACTOR: u64 = US_PER_SEC / US_WAIT;

pub trait Timer {
    /// Sets the minimum interval for the timer, in nanoseconds.
    ///
    /// SAFETY: This function likely assumes interrupts are enabled, in the case
    ///         where external timers must be used to determine frequency. Additionally,
    ///         it is assumed the APIC is both hardware and software enabled.
    ///
    ///         If these conditions are not met, it's possible this function will simply
    ///         hang, doing nothing while waiting for timer interrupts that won't occur.
    // unsafe fn set_frequency(&mut self, set_freq: u16);

    /// Reloads the timer with the given interval multiplier.
    ///
    /// SAFETY: Caller must ensure reloading the timer will not adversely affect regular
    ///         control flow.
    unsafe fn set_next_wait(&mut self, interval_multiplier: u16);
}

/// APIC timer utilizing the built-in APIC clock in one-shot mode.
struct APICTimer(u32);

impl APICTimer {
    /// Creates a new APIC built-in clock timer, in one-shot mode.
    ///
    /// SAFETY: Caller must ensure that reconfiguring the APIC timer mode will not adversely
    ///         affect software execution, and additionally that the [`crate::interrupts::Vector::LocalTimer`] has
    ///         a proper handler.
    pub unsafe fn new(set_freq: u16) -> Option<Self> {
        if *apic::xAPIC_SUPPORT || *apic::x2APIC_SUPPORT {
            apic::get_timer().set_mode(apic::TimerMode::OneShot);

            // TODO perhaps check the state of APIC timer LVT? It should be asserted that the below will always work.
            //      Really, in general, the state of the APIC timer should be more carefully controlled. Perhaps this
            //      can be done when the interrupt device is abstracted out into `libkernel`.

            let freq = {
                let clock = clock::get();
                apic::set_timer_initial_count(u32::MAX);
                clock.spin_wait_us(US_WAIT as u32);
                let timer_count = apic::get_timer_current_count();

                ((u32::MAX - timer_count) as u64) * US_FREQ_FACTOR
            };

            Some(Self((freq / (set_freq as u64)) as u32))
        } else {
            None
        }
    }
}

impl Timer for APICTimer {
    unsafe fn set_next_wait(&mut self, interval_multiplier: u16) {
        assert!(self.0 > 0, "timer frequency has not been configured");

        let timer_wait = self.0.checked_mul(interval_multiplier as u32).expect("timer interval multiplier overflowed");

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
    pub unsafe fn new(set_freq: u16) -> Option<Self> {
        if (*apic::xAPIC_SUPPORT || *apic::x2APIC_SUPPORT)
            && libkernel::cpu::FEATURE_INFO.has_tsc()
            && libkernel::cpu::FEATURE_INFO.has_tsc_deadline()
        {
            apic::get_timer().set_mode(apic::TimerMode::TSC_Deadline);

            let freq = libkernel::cpu::CPUID
                .get_processor_frequency_info()
                .map(|info| {
                    (info.bus_frequency() as u64)
                        / ((info.processor_base_frequency() as u64) * (info.processor_max_frequency() as u64))
                })
                .unwrap_or_else(|| {
                    trace!("CPU does not support TSC frequency reporting via CPUID.");

                    let clock = clock::get();
                    let start_tsc = core::arch::x86_64::_rdtsc();
                    clock.spin_wait_us(US_WAIT as u32);
                    let end_tsc = core::arch::x86_64::_rdtsc();

                    (end_tsc - start_tsc) * US_FREQ_FACTOR
                });

            Some(Self(freq / (set_freq as u64)))
        } else {
            None
        }
    }
}

impl Timer for TSCTimer {
    unsafe fn set_next_wait(&mut self, interval_multiplier: u16) {
        assert!(self.0 > 0, "timer frequency has not been configured");

        let tsc_wait = self.0.checked_mul(interval_multiplier as u64).expect("timer interval multiplier overflowed");

        libkernel::registers::msr::IA32_TSC_DEADLINE::set(libkernel::registers::TSC::read() + tsc_wait);
    }
}
