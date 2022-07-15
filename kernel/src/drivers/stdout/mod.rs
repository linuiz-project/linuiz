mod serial;

pub use serial::*;

use core::fmt::Write;
use liblz::io::port::WriteOnlyPort;
use spin::Mutex;

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

type WriteOption = Option<&'static mut dyn Write>;
struct StandardOut(Mutex<WriteOption>);
unsafe impl Send for StandardOut {}
unsafe impl Sync for StandardOut {}
impl StandardOut {
    fn lock(&self) -> spin::MutexGuard<WriteOption> {
        self.0.lock()
    }
}

static STD_OUT: StandardOut = StandardOut(Mutex::new(None));

pub fn set_stdout(
    std_out: &'static mut dyn Write,
    minimum_level: log::LevelFilter,
) -> Result<(), log::SetLoggerError> {
    *STD_OUT.lock() = Some(std_out);

    crate::logging::init_logger(crate::logging::LoggingModes::SERIAL, minimum_level)
}

#[doc(hidden)]
pub fn __std_out(args: core::fmt::Arguments) {
    let mut std_out = STD_OUT.lock();

    match &mut *std_out {
        Some(std_out) => std_out.write_fmt(args).unwrap(),
        None => panic!("STD_OUT has not been configured."),
    };
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::drivers::stdout::__std_out(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
