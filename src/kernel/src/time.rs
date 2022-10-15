#[cfg(target_arch = "x86_64")]
mod clock {
    pub static SYSTEM_CLOCK: spin::Lazy<Clock> = spin::Lazy::new(|| {
        libarch::interrupts::without(|| {
            // TODO support for invariant TSC as clock

            Clock::load().unwrap()
        })
    });

    pub enum Type<'a> {
        Acpi(crate::acpi::Register<'a, u32>),
    }

    pub struct Clock<'a> {
        ty: Type<'a>,
        frequency: u64,
        max_timestamp: u64,
    }

    // SAFETY: Addresses for type values are required to be globally accessible.
    unsafe impl Send for Clock<'_> {}
    // SAFETY: Addresses for type values are required to be globally accessible.
    unsafe impl Sync for Clock<'_> {}

    impl<'a> Clock<'a> {
        fn load() -> Option<Self> {
            if let Some(pm_timer) = crate::acpi::get_platform_info().pm_timer.as_ref()
                && let Some(register) = crate::acpi::Register::new(&pm_timer.base)
            {
                Some(Self {
                    ty: Type::Acpi(register),
                    frequency: 3579545,
                    max_timestamp: (if pm_timer.supports_32bit { u32::MAX } else { 0xFFFFFF }) as u64
                })

            } else {
                None
            }
        }

        pub fn unload(&mut self) {
            match self.ty {
                Type::Acpi(_) => {}
            }
        }

        #[inline(always)]
        pub const fn frequency(&self) -> u64 {
            self.frequency
        }

        #[inline(always)]
        pub const fn max_timestamp(&self) -> u64 {
            self.max_timestamp
        }

        #[inline(always)]
        pub fn get_timestamp(&self) -> u64 {
            match &self.ty {
                Type::Acpi(register) => register.read() as u64,
            }
        }

        /// Spin-waits for the given number of microseconds.
        pub fn spin_wait_us(&self, microseconds: u32) {
            let ticks_per_us = self.frequency() / 1000000;
            let mut total_ticks = (microseconds as u64) * ticks_per_us;
            let mut current_tick = self.get_timestamp();

            while total_ticks > 0 {
                let new_tick = self.get_timestamp();
                total_ticks -= (new_tick.wrapping_sub(current_tick) & self.max_timestamp()).min(total_ticks);
                current_tick = new_tick;

                core::hint::spin_loop();
            }
        }
    }
}

pub(self) const US_PER_SEC: u32 = 1000000;
pub(self) const US_WAIT: u32 = 10000;
pub(self) const US_FREQ_FACTOR: u32 = US_PER_SEC / US_WAIT;

pub use clock::*;
