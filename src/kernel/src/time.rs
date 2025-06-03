pub use clock::*;

#[cfg(target_arch = "x86_64")]
mod clock {
    pub static SYSTEM_CLOCK: spin::Lazy<Clock> = spin::Lazy::new(|| {
        crate::interrupts::without(|| {
            // TODO support for invariant TSC as clock

            Clock::load().unwrap()
        })
    });

    pub enum Type<'a> {
        Acpi(crate::acpi::Register<'a, u32>),
        // Tsc(u64)
    }

    pub struct Clock<'a> {
        ty: Type<'a>,
        frequency: u64,
        max_timestamp: u64,
    }

    // Safety: Addresses for type values are required to be globally accessible.
    unsafe impl Send for Clock<'_> {}
    // Safety: Addresses for type values are required to be globally accessible.
    unsafe impl Sync for Clock<'_> {}

    impl Clock<'_> {
        fn load() -> Option<Self> {
            let platform_info = crate::acpi::PLATFORM_INFO.as_ref()?;
            let platform_info = platform_info.lock();

            if let Some(pm_timer) = platform_info.pm_timer.as_ref()
                && let Some(register) = crate::acpi::Register::new(&pm_timer.base)
            {
                Some(Self {
                    ty: Type::Acpi(register),
                    frequency: 3579545,
                    max_timestamp: u64::from(if pm_timer.supports_32bit {
                        u32::MAX
                    } else {
                        0xFFFFFF
                    }),
                })
            } else {
                None
            }
        }

        // TODO figure out what to do with this function
        // pub fn unload(&mut self) {
        //     match self.ty {
        //         Type::Acpi(_) => {}
        //     }
        // }

        #[inline]
        pub const fn frequency(&self) -> u64 {
            self.frequency
        }

        #[inline]
        pub const fn max_timestamp(&self) -> u64 {
            self.max_timestamp
        }

        #[inline]
        pub fn get_timestamp(&self) -> u64 {
            match &self.ty {
                Type::Acpi(register) => u64::from(register.read()),
            }
        }

        /// Spin-waits for the given number of microseconds.
        pub fn spin_wait_us(&self, microseconds: u32) {
            let ticks_per_us = self.frequency() / 1000000;
            let mut total_ticks = u64::from(microseconds) * ticks_per_us;
            let mut current_tick = self.get_timestamp();

            while total_ticks > 0 {
                let new_tick = self.get_timestamp();
                total_ticks -=
                    (new_tick.wrapping_sub(current_tick) & self.max_timestamp()).min(total_ticks);
                current_tick = new_tick;

                core::hint::spin_loop();
            }
        }
    }
}
