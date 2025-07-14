use core::arch::x86_64::_rdtsc;

use crate::{arch::x86_64::{
    cpuid::{
        advanced_power_management_info, feature_info, hypervisor_info, processor_frequency_info,
    },
    devices::x2apic::local_vector::{LocalVector, Timer, TimerMode},
}, time::Stopwatch};
use raw_cpuid::{ApmInfo, FeatureInfo, HypervisorInfo};

enum ClockSource {
    TimestampCounter,
    LocalApic,
}

pub struct ArchTimer {
    frequency: u64,
    source: ClockSource,
}

impl ArchTimer {
    pub fn configure() -> Self {
        if feature_info().is_some_and(FeatureInfo::has_tsc)
            && feature_info().is_some_and(FeatureInfo::has_tsc_deadline)
            && advanced_power_management_info().is_some_and(ApmInfo::has_invariant_tsc)
        {
            trace!("System Clock: Timestamp Counter");

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
                .unwrap_or_else(|| {
                    trace!("Processor does not support TSC frequency reporting via `CPUID`.");

                    // Enable the APIC to start the timer (timer is still masked to avoid firing)
                    LocalVector::<Timer>::set_masked(false);

                    // Safety: Processor has TSC capability.
                    let start_tsc = unsafe { _rdtsc() };
                    crate::clock::SYSTEM_CLOCK.spin_wait_us(50000);
                    // Safety: Processor has TSC capability.
                    let end_tsc = unsafe { _rdtsc() };

                    (end_tsc - start_tsc) * US_FREQ_FACTOR
                });

            LocalVector::<Timer>::set_mode(TimerMode::TscDeadline);

            Self {
                frequency,
                source: ClockSource::TimestampCounter,
            }
        } else {
            // We'll have to use the LAPIC, since TSC isn't supported in such a way as to allow it to be useful.

            trace!("System Clock: LAPIC Timer (one-shot)");

            let frequency = hypervisor_info()
                .and_then(raw_cpuid::HypervisorInfo::apic_frequency)
                .map_or_else(
                    || {
                    trace!("Processor does not support local APIC timer frequency reporting via `CPUID`.");

                    // Enable the APIC to start the timer (timer is still masked to avoid firing)
                    LocalVector::<Timer>::set_masked(false);

                    // Safety: Processor has TSC capability.
                    let start_tsc = unsafe { _rdtsc() };
                    Stopwatch::spin_wait_ms(50);
                    // Safety: Processor has TSC capability.
                    let end_tsc = unsafe { _rdtsc() };

                    (end_tsc - start_tsc) * US_FREQ_FACTOR
                    },
                    u64::from,
                );

            Self {
                frequency,
                source: ClockSource::LocalApic,
            }
        }
    }
}

fn wait_stopwatch(microseconds: u32) {
    let timer = crate::acpi::get_root_table(rsdp_request);
}
