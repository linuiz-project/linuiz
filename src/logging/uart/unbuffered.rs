use super::Uart;
use crate::logging::with_formatted_log_record;
use core::{fmt::Write, num::NonZero};
use uart::{LineStatus, address::PortAddress};

const UART_FIFO_SIZE: usize = 16;

pub struct Logger;

impl Logger {
    /// Initializes the UART-based serial logging device.
    pub fn init() -> &'static Self {
        let address = {
            cfg_select! {
                target_arch = "x86_64" => {
                    // TODO allow specifying the serial port in the kernel parameters?
                    let port_address = NonZero::<u16>::new(0x3F8).unwrap();
                    // Safety: 0x3F8 is *very likely* to be the correct serial port; even
                    //         if not, there's no way to check.
                    unsafe { PortAddress::new(port_address) }
                }

                _ => { todo!() }
            }
        };

        super::configure_uart(address);

        &Self
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        cfg_select! {
                debug_assertions  => { metadata.level() <= log::Level::Trace }
            not(debug_assertions) => { metadata.level() <= log::Level::Debug }
        }
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            with_formatted_log_record(record, |args| {
                super::with_uart(|uart| {
                    let mut sync_writer = SyncWriter(uart);
                    sync_writer.write_fmt(args).unwrap();
                });
            });
        }
    }

    fn flush(&self) {
        unimplemented!()
    }
}

struct SyncWriter<'a>(&'a mut Uart);

impl SyncWriter<'_> {
    fn wait_for_empty(&mut self) {
        while !self.0.read_line_status().contains(LineStatus::THR_EMPTY) {
            core::hint::spin_loop();
        }
    }
}

impl core::fmt::Write for SyncWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for (index, c) in s.chars().enumerate() {
            // Wait for the FIFO to empty initially and every 16 bytes written.
            if index.is_multiple_of(UART_FIFO_SIZE) {
                self.wait_for_empty();
            }

            self.0.write_byte(u8::try_from(c).unwrap_or(b'?'));
        }

        Ok(())
    }

    fn write_char(&mut self, c: char) -> core::fmt::Result {
        self.wait_for_empty();
        self.0.write_byte(u8::try_from(c).unwrap_or(b'?'));

        Ok(())
    }
}
