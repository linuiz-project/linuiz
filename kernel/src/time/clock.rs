use acpi::platform::address::AddressSpace;
use libkernel::{io::port::ReadOnlyPort, memory::volatile::VolatileCell};

pub trait Clock {
    /// Unloads the clock, if supported. This is run when the clock is switched out.
    fn unload(&mut self);

    /// Retrieves the clock's current frequency.
    fn get_frequency(&self) -> u64;

    /// Retrieves the current timestamp from the clock.
    fn get_timestamp(&self) -> u64;
}

struct ClockWrapper(Option<alloc::boxed::Box<dyn Clock>>);
unsafe impl Send for ClockWrapper {}
unsafe impl Sync for ClockWrapper {}

static SYSTEM_CLOCK: spin::RwLock<ClockWrapper> = spin::RwLock::new(ClockWrapper(None));

/// Sets the given [`Clock`] as the global system clock.
///
/// SAFETY: If this function is called within an interrupt context, a deadlock may occur.
pub unsafe fn set_system_clock(clock: alloc::boxed::Box<dyn Clock>) {
    libkernel::instructions::interrupts::without_interrupts(|| {
        let mut sys_clk_rw = SYSTEM_CLOCK.write();
        *sys_clk_rw = ClockWrapper(Some(clock));
    })
}

/// Returns the frequency of the current system clock. Panics if there is no system clock.
pub fn get_frequency() -> u64 {
    let system_clock = SYSTEM_CLOCK.read();
    system_clock.0.as_ref().unwrap().get_frequency()
}

/// Returns the timestamp of the current system clock. Panics if there is no system clock.
pub fn get_timestamp() -> u64 {
    let system_clock = SYSTEM_CLOCK.read();
    system_clock.0.as_ref().unwrap().get_timestamp()
}

enum TimerData<'a> {
    IO(ReadOnlyPort<u32>),
    MMIO(&'a VolatileCell<u32, libkernel::ReadOnly>),
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
pub enum ACPIClockError {
    UnsupportedAddressSpace(AddressSpace),
    TimerUnsupported,
    Error(acpi::AcpiError),
}

pub struct ACPIClock<'a>(TimerData<'a>);

impl ACPIClock<'_> {
    /// Loads the ACPI timer, and creates a [`Clock`] from it.
    pub fn load() -> Result<Self, ACPIClockError> {
        unsafe {
            match crate::tables::acpi::get_fadt().pm_timer_block() {
                Ok(Some(timer_block)) if timer_block.address_space == AddressSpace::SystemIo => {
                    Ok(Self(TimerData::IO(ReadOnlyPort::<u32>::new(timer_block.address as u16))))
                }

                Ok(Some(timer_block)) if timer_block.address_space == AddressSpace::SystemMemory => {
                    Ok(Self(TimerData::MMIO(&*(timer_block.address as *const _))))
                }

                Ok(Some(timer_block)) => Err(ACPIClockError::UnsupportedAddressSpace(timer_block.address_space)),
                Ok(None) => Err(ACPIClockError::TimerUnsupported),
                Err(err) => Err(ACPIClockError::Error(err)),
            }
        }
    }
}

impl Clock for ACPIClock<'_> {
    fn unload(&mut self) {
        // ... doesn't ever need to be unloaded
    }

    #[inline(always)]
    fn get_frequency(&self) -> u64 {
        // Frequency pulled from ACPI spec as the below value.
        3579545
    }

    #[inline(always)]
    fn get_timestamp(&self) -> u64 {
        self.0.read() as u64
    }
}
