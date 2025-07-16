#![allow(clippy::similar_names)]

use core::{num::NonZero, ptr::NonNull, time::Duration};
use ioports::ReadOnlyPort;
use safe_mmio::{UniqueMmioPointer, fields::ReadPure};

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
            }
            | Source::AcpiMmio {
                address: _,
                max_value,
            } => *max_value,
        }
    }
}

crate::singleton! {
    pub Stopwatch {
        source: Source,
        ticks_per_sec: u64,
        ticks_per_ms: u64,
        ticks_per_us: u64,
    }

    fn init(rsdp_request: &limine::request::RsdpRequest) {
        if let Ok(acpi_root_table) = crate::acpi::get_root_table(rsdp_request)
            && let Ok(acpi_platform_info) = acpi_root_table.platform_info()
            && let Some(pm_timer) = acpi_platform_info.pm_timer
        {
            trace!("Found ACPI power management timer.");

            match pm_timer.base.address_space {
                acpi::address::AddressSpace::SystemIo => {
                    trace!(
                        "Using ACPI power management timer via port IO: {{ address: {:#X}, is 32 bit: {} }}",
                        pm_timer.base.address, pm_timer.supports_32bit
                    );

                    // TODO potentially use `NonZero<u16>` instead of just `u16`?
                    let port_address =
                        u16::try_from(pm_timer.base.address).expect("invalid port address");

                    Self {
                        source: Source::AcpiIo {
                            // Safety: ACPI spec (and the crate) guarantees the address will be a valid IO port.
                            address: unsafe { ReadOnlyPort::new(port_address) },
                            max_value: if pm_timer.supports_32bit {
                                0xFFFF_FFFF
                            } else {
                                0x00FF_FFFF
                            },
                        },
                        ticks_per_sec: 3579545,
                        ticks_per_ms: 3579545 / 1000,
                        ticks_per_us: 3579545 / 1000 / 1000,
                    }
                }

                acpi::address::AddressSpace::SystemMemory => {
                    trace!(
                        "Using ACPI power management timer via MMIO: {{ address: {:#X}, is 32 bit: {} }}",
                        pm_timer.base.address, pm_timer.supports_32bit
                    );

                    let mmio_address = usize::try_from(pm_timer.base.address)
                        .expect("failed to convert ACPI power management timer address");
                    let mmio_address = NonNull::with_exposed_provenance(
                        NonZero::try_from(mmio_address)
                            .expect("ACPI power management timer address is invalid"),
                    );

                    Self {
                        source: Source::AcpiMmio {
                            // Safety: ACPI spec (and the crate) guarantees the address will be a valid IO port.
                            address: unsafe { UniqueMmioPointer::new(mmio_address) },
                            max_value: if pm_timer.supports_32bit {
                                0xFFFF_FFFF
                            } else {
                                0x00FF_FFFF
                            },
                        },
                        ticks_per_sec: 3579545,
                        ticks_per_ms: 3579545 / 1000,
                        ticks_per_us: 3579545 / 1000 / 1000,
                    }
                }

                _ => unreachable!(),
            }
        } else {
            unimplemented!("only the ACPI power management timer is available as a stopwatch")
        }
    }
}

// Safety: For `Source::Acpi`, references memory mapped in all address spaces.
unsafe impl Send for Stopwatch {}
// Safety: Type is read-only after being constructed.
unsafe impl Sync for Stopwatch {}

impl Stopwatch {
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
