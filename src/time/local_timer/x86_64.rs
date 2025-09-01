use crate::{
    arch::x86_64::{
        cpuid::{
            advanced_power_management_info, feature_info, hypervisor_info, processor_frequency_info,
        },
        devices::local_apic::{LocalApic, TimerDivideConfiguration, local_vector::TimerMode},
        registers::model_specific::IA32_TSC_DEADLINE,
    },
    time::KernelStopwatch,
};
use core::{arch::x86_64::_rdtsc, time::Duration};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("wait duration was too long")]
    InvalidWait,
}

/// Duration to measure other timer sources against [`Stopwatch`].
const MEASUREMENT_DURATION: Duration = Duration::from_millis(50);

/// Amount you need to multiply measured ticks by when using
/// [`MEASUREMENT_DURATION`].
#[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
const MEASUREMENT_FREQUENCY_FACTOR: u32 =
    (Duration::SECOND.as_micros() / MEASUREMENT_DURATION.as_micros()) as u32;

fn measure_tsc() -> u64 {
    trace!("Measuring the timestamp counter frequency...");

    // Safety: Processor has TSC capability.
    let start_tsc = unsafe { _rdtsc() };
    KernelStopwatch::spin_wait(MEASUREMENT_DURATION);
    // Safety: Processor has TSC capability.
    let end_tsc = unsafe { _rdtsc() };

    let elapsed_ticks = end_tsc - start_tsc;
    let frequency = elapsed_ticks * u64::from(MEASUREMENT_FREQUENCY_FACTOR);

    trace!("Timestamp counter frequency: {frequency}Hz");

    frequency
}

fn measure_lapic() -> u32 {
    trace!("Measuring the local APIC timer frequency...");

    LocalApic::set_timer_divide_configuration(TimerDivideConfiguration::DivideBy1);

    const MEASURE_TIMER_COUNTDOWN_VALUE: u32 = u32::MAX;

    // Loading the initial count starts the timer.
    LocalApic::set_timer_initial_count(MEASURE_TIMER_COUNTDOWN_VALUE);
    KernelStopwatch::spin_wait(MEASUREMENT_DURATION);
    let end_timer_count = LocalApic::get_timer_current_count();

    let elapsed_ticks = MEASURE_TIMER_COUNTDOWN_VALUE - end_timer_count;
    let frequency = elapsed_ticks * MEASUREMENT_FREQUENCY_FACTOR;

    trace!("Local APIC timer frequency: {frequency}Hz");

    frequency
}

#[derive(Debug)]
enum Mode {
    TscDeadline {
        ticks_per_sec: u64,
        ticks_per_ms: u64,
        ticks_per_us: u64,
    },
    OneShot {
        ticks_per_sec: u32,
        ticks_per_ms: u32,
        ticks_per_us: u32,
    },
}

pub struct LocalTimer(Mode);

impl LocalTimer {
    pub fn configure() -> Self {
        let mode = {
            if feature_info().is_some_and(|cpuid| cpuid.has_tsc())
                && feature_info().is_some_and(|cpuid| cpuid.has_tsc_deadline())
                && advanced_power_management_info().is_some_and(|cpuid| cpuid.has_invariant_tsc())
            {
                trace!("Local Timer: Timestamp Counter");

                LocalApic::lvt_timer().set_mode(TimerMode::TscDeadline);

                // Notably, on AMD systems the first check simply won't work, becuase AMD is
                // cursed and Lisa Su is continuing AMD's time-honored tradition of
                // making their CPUs 10x more difficult to program for than Intel.
                let frequency = processor_frequency_info()
                    .map(|processor_frequency_info| {
                        // We read the processor frequency information directly from the CPU, to do
                        // the math to make it useful.
                        u64::from(processor_frequency_info.bus_frequency())
                            / (u64::from(processor_frequency_info.processor_base_frequency())
                                * u64::from(processor_frequency_info.processor_max_frequency()))
                    })
                    .or_else(|| {
                        // Check if we're in a hypervisor environment and it provides the 0x40000000
                        // and 0x40000010 hypervisor info leaves.
                        feature_info()
                            .filter(raw_cpuid::FeatureInfo::has_hypervisor)
                            .and_then(|_| hypervisor_info().and_then(|cpuid| cpuid.tsc_frequency()))
                            .map(u64::from)
                    })
                    .unwrap_or_else(measure_tsc);

                Mode::TscDeadline {
                    ticks_per_sec: frequency,
                    ticks_per_ms: frequency / 1_000,
                    ticks_per_us: frequency / 1_000_000,
                }
            } else {
                // We'll have to use the LAPIC, since TSC isn't supported in such a way as to
                // allow it to be useful.

                trace!("Local Timer: APIC (one-shot)");

                LocalApic::lvt_timer().set_mode(TimerMode::OneShot);

                let frequency = hypervisor_info()
                    .and_then(|cpuid| cpuid.apic_frequency())
                    .unwrap_or_else(measure_lapic);

                Mode::OneShot {
                    ticks_per_sec: frequency,
                    ticks_per_ms: frequency / 1_000,
                    ticks_per_us: frequency / 1_000_000,
                }
            }
        };

        trace!("Local Timer: {mode:?}");

        Self(mode)
    }

    /// Enables the timer interrupt.
    #[allow(clippy::unused_self)]
    pub fn enable(&mut self) {
        LocalApic::lvt_timer().set_masked(false);
    }

    /// Disables the timer interrupt.
    #[allow(clippy::unused_self)]
    pub fn disable(&mut self) {
        LocalApic::lvt_timer().set_masked(true);
    }

    /// Sets a timer for `duration`, which upon elapsing will fire an interrupt.
    pub fn set_wait(&mut self, duration: Duration) -> Result<(), Error> {
        match self.0 {
            Mode::TscDeadline {
                ticks_per_sec: _,
                ticks_per_ms: _,
                ticks_per_us,
            } => {
                let wait_us =
                    u64::try_from(duration.as_micros()).map_err(|_| Error::InvalidWait)?;
                let wait_ticks = ticks_per_us
                    .checked_mul(wait_us)
                    .ok_or(Error::InvalidWait)?;

                trace!("Wait (TSC): {{ {wait_us:?}us, {wait_ticks}t }} ");
                // Safety: If mode is `TscDeadline`, then the timestamp counter
                //         is supported.
                unsafe {
                    IA32_TSC_DEADLINE::set(wait_ticks);
                }
            }

            Mode::OneShot {
                ticks_per_sec: _,
                ticks_per_ms: _,
                ticks_per_us,
            } => {
                let wait_us =
                    u32::try_from(duration.as_micros()).map_err(|_| Error::InvalidWait)?;
                let wait_ticks = ticks_per_us
                    .checked_mul(wait_us)
                    .ok_or(Error::InvalidWait)?;

                trace!("Wait (APIC): {{ {wait_us:?}us, {wait_ticks}t }} ");
                LocalApic::set_timer_initial_count(wait_ticks);
            }
        }

        Ok(())
    }

    pub fn set_preemption_wait(&mut self) {
        self.set_wait(Duration::from_millis(15)).unwrap();
    }
}
