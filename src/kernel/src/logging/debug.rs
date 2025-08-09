use crate::interrupts::InterruptCell;
use core::fmt::Write;
use ioports::{ReadOnlyPort, WriteOnlyPort};
use log::{Level, Log, Metadata, Record};
use spin::{Mutex, Once};

/// A debug output utilizing QEMU's port 0xE9 hack.
pub struct Logger(Option<InterruptCell<Mutex<Writer>>>);

impl Logger {
    const PORT_ADDRESS: u16 = 0xE9;

    /// Initialize the QEMU 0xE9-hack debug logger.
    ///
    /// Subsequent calls after the first will do nothing but return a reference
    /// to the static logger.
    pub fn init() -> &'static Self {
        static DEBUG_LOGGER: Once<Logger> = Once::new();

        DEBUG_LOGGER.call_once(|| {
            #[cfg(target_arch = "x86_64")]
            if crate::arch::x86_64::cpuid::hypervisor_info().is_none() {
                return Self(None);
            }

            // Safety: We're testing if the port exists.
            let test_port = unsafe { ReadOnlyPort::<u8>::new(0xE9) };
            if test_port.read() == 0xE9 {
                // Safety: If a read on port 0xE9 returns `0xE9`, then QEMU
                //         guarantees it exists.
                let mut debug_port = unsafe { WriteOnlyPort::<u8>::new(0xE9) };

                b"-DEBUG LOGGER-\n"
                    .iter()
                    .for_each(|character| debug_port.write(*character));

                Self(Some(InterruptCell::new(Mutex::new(Writer(debug_port)))))
            } else {
                Self(None)
            }
        })
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            super::with_formatted_log_record(record, |args| {
                self.0.as_ref().inspect(|writer| {
                    writer.with(|writer| {
                        let mut writer = writer.lock();

                        writer.write_fmt(args).ok();
                    });
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
