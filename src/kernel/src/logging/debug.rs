use crate::interrupts::InterruptCell;
use core::fmt::Write;
use ioports::WriteOnlyPort;
use spin::{Mutex, Once};

/// A debug output utilizing QEMU's port 0xE9 hack.
pub struct Logger(InterruptCell<Mutex<Writer>>);

impl Logger {
    /// Initialized the QEMU 0xE9-hack debug logger.
    ///
    /// Subsequent calls after the first will do nothing but return a reference to the static logger.
    pub fn init() -> &'static Self {
        static DEBUG_LOGGER: Once<Logger> = Once::new();

        DEBUG_LOGGER.call_once(|| {
            Self(InterruptCell::new(Mutex::new(Writer({
                // Safety: It's assumed that this port exists if the kernel was compiled and run in debug mode.
                unsafe { WriteOnlyPort::new(0xE9) }
            }))))
        })
    }
}

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            super::with_formatted_log_record(record, |args| {
                self.0.with(|writer| {
                    let mut writer = writer.lock();

                    writer.write_fmt(args).ok();
                });
            });
        }
    }

    fn flush(&self) {
        unimplemented!()
    }
}

struct Writer(WriteOnlyPort<u8>);

impl core::fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars()
            .map(|c| u8::try_from(c).unwrap_or(b'?'))
            .for_each(|byte| self.0.write(byte));

        Ok(())
    }

    fn write_char(&mut self, c: char) -> core::fmt::Result {
        self.0.write(u8::try_from(c).unwrap_or(b'?'));

        Ok(())
    }
}
