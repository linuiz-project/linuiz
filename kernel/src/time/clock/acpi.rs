use crate::tables::acpi;
/// Clock wrapper around the ACPI PWM timer.
pub struct AcpiClock<'a>(acpi::Register<'a, u32>);

unsafe impl Send for AcpiClock<'_> {}
unsafe impl Sync for AcpiClock<'_> {}

impl AcpiClock<'_> {
    // Frequency pulled from ACPI spec as the below value.
    const FREQUENCY: u32 = 3579545;

    /// Loads the ACPI timer, and creates a [`Clock`] from it.
    pub fn load() -> Option<Self> {
        unsafe {
            let fadt = crate::tables::acpi::get_fadt();

            fadt.pm_timer_block()
                .ok()
                .and_then(|f| f)
                .and_then(|timer_block| acpi::Register::new(&timer_block))
                .map(|register| Self(register))
        }
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
}
