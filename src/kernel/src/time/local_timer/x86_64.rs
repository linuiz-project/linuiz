use crate::{
    arch::x86_64::{
        cpuid::{
            advanced_power_management_info, feature_info, hypervisor_info, processor_frequency_info,
        },
        devices::x2apic::{local_vector::TimerMode, x2Apic},
    },
    time::Stopwatch,
};
use core::{arch::x86_64::_rdtsc, time::Duration};
use raw_cpuid::{ApmInfo, FeatureInfo, HypervisorInfo};

enum Source {
    TimestampCounter,
    LocalApic,
}

pub struct LocalTimer {
    frequency: u64,
    source: Source,
}

impl LocalTimer {
    pub fn configure() -> Self {
        if feature_info().is_some_and(FeatureInfo::has_tsc)
            && feature_info().is_some_and(FeatureInfo::has_tsc_deadline)
            && advanced_power_management_info().is_some_and(ApmInfo::has_invariant_tsc)
        {
            trace!("Local Timer: Timestamp Counter");

            x2Apic::lvt_timer().set_mode(TimerMode::TscDeadline);

            // Notably, on AMD systems the first check simply won't work, becuase AMD is cursed and Lisa Su is
            // continuing AMD's time-honored tradition of making their CPUs 10x more difficult to program for than Intel.
            let frequency = processor_frequency_info()
                .map(|processor_frequency_info| {
                    // We read the processor frequency information directly from the CPU, to do the math to make it useful.
                    u64::from(processor_frequency_info.bus_frequency())
                        / (u64::from(processor_frequency_info.processor_base_frequency())
                            * u64::from(processor_frequency_info.processor_max_frequency()))
                })
                .or_else(|| {
                    // We're in a hypervisor environment and it provides the 0x40000000 and 0x40000010 hypervisor info leaves.
                    feature_info()
                        .is_some_and(FeatureInfo::has_hypervisor)
                        .then(|| hypervisor_info())
                        .flatten()
                        .and_then(HypervisorInfo::tsc_frequency)
                        .map(u64::from)
                })
                .unwrap_or_else(measure_tsc);

            Self {
                frequency,
                source: Source::TimestampCounter,
            }
        } else {
            // We'll have to use the LAPIC, since TSC isn't supported in such a way as to allow it to be useful.

            trace!("Local Timer: APIC (one-shot)");

            x2Apic::lvt_timer().set_mode(TimerMode::OneShot);

            let frequency = hypervisor_info()
                .and_then(raw_cpuid::HypervisorInfo::apic_frequency)
                .map_or_else(measure_lapic, u64::from);

            Self {
                frequency,
                source: Source::LocalApic,
            }
        }
    }
}

/// Duration to measure other timer sources against [`Stopwatch`].
const MEASUREMENT_DURATION: Duration = Duration::from_millis(50);

/// Amount you need to multiply measured ticks by when using [`MEASUREMENT_DURATION`].
#[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
const MEASUREMENT_FREQUENCY_FACTOR: u32 =
    (Duration::SECOND.as_micros() / MEASUREMENT_DURATION.as_micros()) as u32;

fn measure_tsc() -> u64 {
    trace!("Measuring the timestamp counter frequency...");

    // Safety: Processor has TSC capability.
    let start_tsc = unsafe { _rdtsc() };
    Stopwatch::spin_wait(MEASUREMENT_DURATION);
    // Safety: Processor has TSC capability.
    let end_tsc = unsafe { _rdtsc() };

    let elapsed_ticks = end_tsc - start_tsc;
    let frequency = elapsed_ticks * u64::from(MEASUREMENT_FREQUENCY_FACTOR);

    trace!("Timestamp counter frequency: {frequency}Hz");

    frequency
}

fn measure_lapic() -> u64 {
    trace!("Measuring the local APIC timer frequency...");

    x2Apic::set_timer_divide_configuration(
        crate::arch::x86_64::devices::x2apic::TimerDivideConfiguration::DivideBy1,
    );

    const MEASURE_TIMER_COUNTDOWN_VALUE: u32 = u32::MAX;

    // Loading the initial count starts the timer.
    x2Apic::set_timer_initial_count(MEASURE_TIMER_COUNTDOWN_VALUE);
    Stopwatch::spin_wait(MEASUREMENT_DURATION);
    let end_timer_count = x2Apic::get_timer_current_count();

    let elapsed_ticks = MEASURE_TIMER_COUNTDOWN_VALUE - end_timer_count;
    let frequency = u64::from(elapsed_ticks * MEASUREMENT_FREQUENCY_FACTOR);

    trace!("Local APIC timer frequency: {frequency}Hz");

    frequency
}
