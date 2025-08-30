use crate::util::sync::{Mutex, Once};
use ioports::{ReadOnlyPort, WriteOnlyPort};

pub struct Logger(Mutex<Writer>);

static DEBUG_LOGGER: Once<Logger> = Once::new();

impl Logger {
    pub fn init() -> Option<&'static Self> {
        DEBUG_LOGGER
            .try_call_once(|| {
                // Safety: We're testing if the port exists.
                let test_port = unsafe { ReadOnlyPort::<u8>::new(0xE9) };
                if test_port.read() == 0xE9 {
                    // Safety: If a read at 0xE9 returns `0xE9`, then QEMU guarantees it exists.
                    let mut debug_port = unsafe { WriteOnlyPort::<u8>::new(0xE9) };

                    b"-DEBUG LOGGER-\n"
                        .iter()
                        .for_each(|byte| debug_port.write(*byte));

                    Ok(Self(Mutex::new(Writer(debug_port))))
                } else {
                    Err(())
                }
            })
            .ok()
    }
}

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        super::with_formatted_log_record(record, |args| {
            self.0.with_lock(|writer| {
                core::fmt::Write::write_fmt(writer, args).ok();
            });
        });
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
