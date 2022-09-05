mod acpi;

use alloc::boxed::Box;

pub trait Clock: Send + Sync {
    /// Unloads the clock, if supported. This is run when the clock is switched out.
    fn unload(&mut self);

    /// Retrieves the clock's current frequency.
    fn get_frequency(&self) -> u64;

    /// Retrieves the current timestamp from the clock.
    fn get_timestamp(&self) -> u64;

    fn get_max_timestamp(&self) -> u64;

    /// Spin-waits for the given number of microseconds.
    fn spin_wait_us(&self, microseconds: u32) {
        let ticks_per_us = self.get_frequency() / 1000000;
        let mut total_ticks = (microseconds as u64) * ticks_per_us;
        let mut current_tick = self.get_timestamp();

        while total_ticks > 0 {
            let new_tick = self.get_timestamp();
            total_ticks -= (new_tick.wrapping_sub(current_tick) & self.get_max_timestamp()).min(total_ticks);
            current_tick = new_tick;

            core::hint::spin_loop();
        }
    }
}

static SYSTEM_CLOCK: spin::Once<Box<dyn Clock>> = spin::Once::new();

/// Sets the given [`Clock`] as the global system clock.
///
/// SAFETY: If this function is called within an interrupt context, a deadlock may occur.
pub fn get() -> &'static Box<dyn Clock> {
    SYSTEM_CLOCK.call_once(|| {
        crate::interrupts::without(|| {
            // TODO support for invariant TSC as clock

            Box::new(acpi::AcpiClock::load().unwrap())
        })
    })
}
