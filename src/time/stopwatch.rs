#![allow(clippy::similar_names)]

use crate::{acpi::fadt::PmTimer, util::sync::Once};
use core::time::Duration;

pub struct KernelStopwatch {
    source: PmTimer,
    ticks_per_sec: u64,
    ticks_per_ms: u64,
    ticks_per_us: u64,
}

static KERNEL_STOPWATCH: Once<KernelStopwatch> = Once::new();

// Safety: For `Source::Acpi`, references memory mapped in all address spaces.
unsafe impl Send for KernelStopwatch {}
// Safety: Type is read-only after being constructed.
unsafe impl Sync for KernelStopwatch {}

impl KernelStopwatch {
    pub fn init(pm_timer: PmTimer) {
        KERNEL_STOPWATCH.call_once(|| {
            trace!("ACPI PWM Timer: {pm_timer:?}");
            trace!("Timer will be used for stopwatch operations.");

            Self {
                source: pm_timer,
                ticks_per_sec: 357_9545,
                ticks_per_ms: 357_9545 / 1000,
                ticks_per_us: 357_9545 / 1000 / 1000,
            }
        });
    }

    fn get_static() -> &'static Self {
        KERNEL_STOPWATCH.get().unwrap()
    }

    /// Spin waits for the provided [`Duration`].
    ///
    /// # Remarks
    ///
    /// - [`Duration`]s greater than [`u64::MAX`] microseconds will be
    ///   truncated.
    pub fn spin_wait(duration: Duration) {
        let stopwatch = Self::get_static();

        let duration_us = u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
        let mut wait_ticks = duration_us * stopwatch.ticks_per_us;
        let mut last_tick_count = stopwatch.source.read();

        while wait_ticks > 0 {
            let current_tick_count = stopwatch.source.read();
            let elapsed_ticks = {
                if last_tick_count < current_tick_count {
                    // ... the counter did not overflow ...

                    current_tick_count - last_tick_count
                } else {
                    // ... the counter overflowed...

                    // Calculates the ticks we lost during the overflow.
                    let overflow_ticks = stopwatch.source.max_value() - last_tick_count;
                    current_tick_count + overflow_ticks
                }
            };

            wait_ticks = wait_ticks.saturating_sub(elapsed_ticks);
            last_tick_count = current_tick_count;

            core::hint::spin_loop();
        }
    }
}
