mod debug_out;
mod serial;

pub use debug_out::*;
pub use serial::*;
use spin::Mutex;

use core::fmt::Write;
use libstd::{cell::SyncRefCell, io::port::WriteOnlyPort};

static STDOUT: SyncRefCell<Mutex<&'static mut dyn Write>> = SyncRefCell::empty();

pub fn set_stdout(
    stdout: &'static mut dyn Write,
    minimum_level: log::LevelFilter,
    trace_enabled_paths: &'static [&'static str],
) -> Result<(), log::SetLoggerError> {
    STDOUT.set(Mutex::new(stdout));

    crate::logging::init_logger(
        crate::logging::LoggingModes::STDOUT,
        minimum_level,
        trace_enabled_paths,
    )
}

#[doc(hidden)]
pub fn __std_out(args: core::fmt::Arguments) {
    if let Some(lock) = STDOUT.borrow() {
        let mut std_out = lock.lock();

        std_out.write_fmt(args).unwrap();
    } else {
        panic!("stdout has not been configured");
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::drivers::io::__std_out(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

pub struct QEMUE9(WriteOnlyPort<u8>);

impl QEMUE9 {
    pub const fn new() -> Self {
        Self(unsafe { WriteOnlyPort::new(0xE9) })
    }
}

impl Write for QEMUE9 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            self.0.write(byte);
        }

        Ok(())
    }
}
