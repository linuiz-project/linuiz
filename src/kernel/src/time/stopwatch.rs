#![allow(clippy::similar_names)]

use core::{num::NonZero, ptr::NonNull, time::Duration};
use ioports::ReadOnlyPort;
use safe_mmio::{UniqueMmioPointer, fields::ReadPure};
use spin::Once;

use crate::acpi::get_root_table;

static GLOBAL_STOPWATCH: Once<Stopwatch> = Once::new();

enum Source {
    AcpiIo {
        address: ReadOnlyPort<u32>,
        max_value: u64,
    },
    AcpiMmio {
        address: UniqueMmioPointer<'static, ReadPure<u32>>,
        max_value: u64,
    },
}

impl Source {
    fn read(&self) -> u64 {
        match self {
            Source::AcpiIo {
                address,
                max_value: _,
            } => u64::from(address.read()),
            Source::AcpiMmio {
                address,
                max_value: _,
            } => u64::from(address.read()),
        }
    }

    fn max_value(&self) -> u64 {
        match self {
            Source::AcpiIo {
                address: _,
                max_value,
            } => *max_value,
            Source::AcpiMmio {
                address: _,
                max_value,
            } => *max_value,
        }
    }
}

pub struct Stopwatch {
    source: Source,
    ticks_per_sec: u64,
    ticks_per_ms: u64,
    ticks_per_us: u64,
}

// Safety: For `Source::Acpi`, references memory mapped in all address spaces.
unsafe impl Send for Stopwatch {}
// Safety: Type is read-only after being constructed.
unsafe impl Sync for Stopwatch {}

impl Stopwatch {
    pub fn init(rsdp_request: &limine::request::RsdpRequest) {
        GLOBAL_STOPWATCH.call_once(|| {
            trace!("Searching system to configure best possible stopwatch.");

            if let Ok(acpi_root_table) = get_root_table(rsdp_request)
                && let Ok(acpi_platform_info) = acpi_root_table.platform_info()
                && let Some(pm_timer) = acpi_platform_info.pm_timer
            {
                trace!("Found ACPI power management timer.");

                let acpi_pm_timer_address = pm_timer.base;
                match acpi_pm_timer_address.address_space {
                    acpi::address::AddressSpace::SystemIo => {
                        // TODO potentially use `NonZero<u16>` instead of just `u16`?
                        let port_address = u16::try_from(acpi_pm_timer_address.address)
                            .expect("invalid port address");

                        let ticks_per_sec = 3579545;
                        let ticks_per_ms = ticks_per_sec / 1000;
                        let ticks_per_us = ticks_per_ms / 1000;

                        Self {
                            source: Source::AcpiIo {
                                // Safety: ACPI spec (and the crate) guarantees the address will be a valid IO port.
                                address: unsafe { ReadOnlyPort::new(port_address) },
                                max_value: if pm_timer.supports_32bit {
                                    0xFFFF_FFFF
                                } else {
                                    0xFFFF_FF00
                                },
                            },
                            ticks_per_sec,
                            ticks_per_ms,
                            ticks_per_us,
                        }
                    }

                    acpi::address::AddressSpace::SystemMemory => {
                        let mmio_address = usize::try_from(acpi_pm_timer_address.address)
                            .expect("failed to convert ACPI power management timer address");
                        let mmio_address = NonNull::with_exposed_provenance(
                            NonZero::try_from(mmio_address)
                                .expect("ACPI power management timer address is invalid"),
                        );

                        let ticks_per_sec = 3579545;
                        let ticks_per_ms = ticks_per_sec / 1000;
                        let ticks_per_us = ticks_per_ms / 1000;

                        Self {
                            source: Source::AcpiMmio {
                                // Safety: ACPI spec (and the crate) guarantees the address will be a valid IO port.
                                address: unsafe { UniqueMmioPointer::new(mmio_address) },
                                max_value: if pm_timer.supports_32bit {
                                    0xFFFF_FFFF
                                } else {
                                    0xFFFF_FF00
                                },
                            },
                            ticks_per_sec,
                            ticks_per_ms,
                            ticks_per_us,
                        }
                    }

                    _ => unreachable!(),
                }
            } else {
                unimplemented!("only the ACPI power management timer is available as a stopwatch")
            }
        });
    }

    fn get_static() -> &'static Self {
        GLOBAL_STOPWATCH.wait()
    }

    /// Spin waits for the provided [`Duration`].
    ///
    /// # Remarks
    ///
    /// - [`Duration`]s greater than [`u64::MAX`] microseconds will be truncated.
    pub fn spin_wait(duration: Duration) {
        let stopwatch = Self::get_static();

        let duration_us = u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
        let mut wait_ticks = duration_us * stopwatch.ticks_per_us;
        let mut last_tick_count = stopwatch.source.read();
        while wait_ticks > 0 {
            let next_tick_count = stopwatch.source.read();
            let (mut elapsed_ticks, is_overflow) = next_tick_count.overflowing_sub(last_tick_count);

            // Collect the portion that we lost in the overflow.
            if is_overflow {
                elapsed_ticks += stopwatch.source.max_value() - last_tick_count;
            }

            wait_ticks = wait_ticks.saturating_sub(elapsed_ticks);
            last_tick_count = next_tick_count;

            core::hint::spin_loop();
        }
    }
}
