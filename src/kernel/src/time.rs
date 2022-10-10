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
                info!("{} - {}", total_ticks, new_tick,);
                total_ticks -= (new_tick.wrapping_sub(current_tick) & self.max_timestamp()).min(total_ticks);
                current_tick = new_tick;
                core::hint::spin_loop();
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
mod timer {
    use libarch::x64::structures::apic;

    enum Type {
        Apic(u32),
        Tsc(u64),
    }

    pub struct Timer(Type);

    impl Timer {
        pub unsafe fn new(freq: u16) -> Option<Self> {
            if libarch::x64::registers::msr::IA32_APIC_BASE::get_hw_enabled() {
                if libarch::x64::cpu::cpuid::FEATURE_INFO.has_tsc()
                    && libarch::x64::cpu::cpuid::FEATURE_INFO.has_tsc_deadline()
                {
                    apic::get_timer().set_mode(apic::TimerMode::TSC_Deadline);

                    let timer_freq = libarch::x64::cpu::cpuid::CPUID.get_processor_frequency_info().map_or_else(
                        || {
                            trace!("CPU does not support TSC frequency reporting via CPUID.");

                            apic::sw_enable();
                            apic::get_timer().set_masked(true).set_mode(apic::TimerMode::TSC_Deadline);

                            let start_tsc = core::arch::x86_64::_rdtsc();
                            crate::time::SYSTEM_CLOCK.spin_wait_us(super::US_WAIT);
                            let end_tsc = core::arch::x86_64::_rdtsc();

                            (end_tsc - start_tsc) * (super::US_FREQ_FACTOR as u64)
                        },
                        |info| {
                            (info.bus_frequency() as u64)
                                / ((info.processor_base_frequency() as u64) * (info.processor_max_frequency() as u64))
                        },
                    );

                    Some(Self(Type::Tsc(timer_freq / (freq as u64))))
                } else {
                    apic::sw_enable();
                    apic::set_timer_divisor(libarch::x64::structures::apic::TimerDivisor::Div1);
                    apic::get_timer().set_masked(true).set_mode(apic::TimerMode::OneShot);

                    let timer_freq = {
                        apic::set_timer_initial_count(u32::MAX);
                        crate::time::SYSTEM_CLOCK.spin_wait_us(super::US_WAIT);
                        let timer_count = apic::get_timer_current_count();

                        (u32::MAX - timer_count) * super::US_FREQ_FACTOR
                    };

                    // Ensure we reset the APIC timer to avoid any errant interrupts.
                    apic::set_timer_initial_count(0);

                    Some(Self(Type::Apic(timer_freq / (freq as u32))))
                }
            } else {
                None
            }
        }

        pub unsafe fn enable(&mut self) {
            apic::get_timer().set_masked(false);
        }

        pub unsafe fn disable(&mut self) {
            apic::get_timer().set_masked(true);
        }

        pub unsafe fn set_next_wait(&mut self, interval_multiplier: u16) {
            match self.0 {
                Type::Apic(interval) => {
                    assert!(interval > 0, "timer frequency has not been determined");

                    apic::set_timer_initial_count(interval * (interval_multiplier as u32));
                }
                Type::Tsc(interval) => {
                    assert!(interval > 0, "timer frequency has not been determined");

                    libarch::x64::registers::msr::IA32_TSC_DEADLINE::set(
                        core::arch::x86_64::_rdtsc() + (interval * (interval_multiplier as u64)),
                    );
                }
            }
        }
    }
}

pub(self) const US_PER_SEC: u32 = 1000000;
pub(self) const US_WAIT: u32 = 10000;
pub(self) const US_FREQ_FACTOR: u32 = US_PER_SEC / US_WAIT;

pub use clock::*;
pub use timer::*;
