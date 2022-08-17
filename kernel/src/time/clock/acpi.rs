use acpi::platform::address::AddressSpace;

enum TimerData<'a> {
    IO(libkernel::io::port::ReadOnlyPort<u32>),
    MMIO(&'a libkernel::memory::volatile::VolatileCell<u32, libkernel::ReadOnly>),
}

impl TimerData<'_> {
    #[inline]
    fn read(&self) -> u32 {
        match self {
            TimerData::IO(port) => port.read(),
            TimerData::MMIO(addr) => addr.read(),
        }
    }
}

#[derive(Debug)]
pub enum AcpiClockError {
    UnsupportedAddressSpace(AddressSpace),
    TimerUnsupported,
    Error(acpi::AcpiError),
}

/// Clock wrapper around the ACPI PWM timer.
pub struct AcpiClock<'a> {
    tmr_mask: u32,
    data: TimerData<'a>,
}

unsafe impl Send for AcpiClock<'_> {}
unsafe impl Sync for AcpiClock<'_> {}

impl AcpiClock<'_> {
    // Frequency pulled from ACPI spec as the below value.
    const FREQUENCY: u32 = 3579545;

    /// Loads the ACPI timer, and creates a [`Clock`] from it.
    pub fn load() -> Result<Self, AcpiClockError> {
        unsafe {
            let fadt = crate::tables::acpi::get_fadt();
            let tmr_mask = if core::ptr::addr_of!(fadt.flags).read_unaligned().pm_timer_is_32_bit() {
                0xFFFFFFFF
            } else {
                0xFFFFFF
            };

            match fadt.pm_timer_block() {
                Ok(Some(timer_block)) if timer_block.address_space == AddressSpace::SystemIo => Ok(Self {
                    tmr_mask,
                    data: TimerData::IO(libkernel::io::port::ReadOnlyPort::<u32>::new(timer_block.address as u16)),
                }),

                Ok(Some(timer_block)) if timer_block.address_space == AddressSpace::SystemMemory => {
                    Ok(Self { tmr_mask, data: TimerData::MMIO(&*(timer_block.address as *const _)) })
                }

                Ok(Some(timer_block)) => Err(AcpiClockError::UnsupportedAddressSpace(timer_block.address_space)),
                Ok(None) => Err(AcpiClockError::TimerUnsupported),
                Err(err) => Err(AcpiClockError::Error(err)),
            }
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
        self.data.read() as u64
    }
}
