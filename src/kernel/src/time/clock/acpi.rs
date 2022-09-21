/// Clock wrapper around the ACPI PWM timer.
#[allow(clippy::module_name_repetitions)]
pub struct AcpiClock<'a>(libcommon::acpi::Register<'a, u32>, u32);

// SAFETY: This structure is effectively read-only with no side-effects.
unsafe impl Send for AcpiClock<'_> {}
// SAFETY: This structure is effectively read-only with no side-effects.
unsafe impl Sync for AcpiClock<'_> {}

impl AcpiClock<'_> {
    // Frequency pulled from ACPI spec as the below value.
    const FREQUENCY: u32 = 3579545;

    /// Loads the ACPI timer, and creates a [`Clock`] from it.
    pub fn load() -> Option<Self> {
        libcommon::acpi::get_platform_info().pm_timer.as_ref().and_then(|pm_timer| {
            libcommon::acpi::Register::new(&pm_timer.base)
                .map(|register| Self(register, if pm_timer.supports_32bit { u32::MAX } else { 0xFFFFFF }))
        })
    }
}

impl super::Clock for AcpiClock<'_> {
    fn unload(&mut self) {
        // ... doesn't ever need to be unloaded
    }

    #[inline(always)]
    fn get_frequency(&self) -> u64 {
        Self::FREQUENCY as u64
    }

    #[inline(always)]
    fn get_timestamp(&self) -> u64 {
        self.0.read() as u64
    }

    #[inline(always)]
    fn get_max_timestamp(&self) -> u64 {
        self.1 as u64
    }
}
