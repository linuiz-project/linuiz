use core::fmt::Write;
use spin::Once;

pub struct QEMUE9(crate::memory::io::WriteOnlyPort<u8>);

impl QEMUE9 {
    pub const fn new() -> Self {
        Self(unsafe { crate::memory::io::WriteOnlyPort::new(0xE9) })
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

#[doc(hidden)]
pub fn __std_out(args: core::fmt::Arguments) {
    crate::interrupts::without(|| {
        STD_OUT.get().unwrap().0.lock().write_fmt(args).unwrap();
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::stdout::__std_out(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! newline {
    () => {
        $crate::print!("\n")
    };
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct Logger : u8 {
        const NONE = 0;
        const SERIAL = 1 << 0;
        const GRAPHIC = 1 << 1;
    }
}

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let ticks = 0;
            let whole_time = ticks / 1000;
            let frac_time = ticks % 1000;

            // TODO possibly only log CPU# in debug builds (or debug/trace messages?)
            crate::println!(
                // TODO restore cpu ID logging
                // "[{whole_time:wwidth$}.{frac_time:0fwidth$}][CPU{cpu_id}][{level}] {args}",
                "[{whole_time:wwidth$}.{frac_time:0fwidth$}][{level}] {args}",
                //cpu_id = crate::cpu::get_id(),
                level = record.level(),
                args = record.args(),
                wwidth = 4,
                fwidth = 3
            );
        }
    }

    fn flush(&self) {}
}

static LOGGER: core::cell::SyncUnsafeCell<Logger> = core::cell::SyncUnsafeCell::new(Logger::empty());

/// Initializes the kernel logger with the provided modes, using the given minimum logging level to filter logs.
///
/// SAFETY: Calling this method more than once could result in undocumented behaviour.
unsafe fn init_logger(modes: Logger, min_level: log::LevelFilter) {
    crate::interrupts::without(|| {
        *LOGGER.get() = modes;

        match unsafe { log::set_logger_racy(&*LOGGER.get()) } {
            Ok(()) => log::set_max_level(min_level),
            Err(error) => panic!("error initializing logger: {:?}", error),
        }
    });
}

struct StdOutWrapper(spin::Mutex<&'static mut dyn Write>);
// SAFETY: Type wraps a spinning mutex, to avoid multiple writes at once.
unsafe impl Send for StdOutWrapper {}
// SAFETY: Type wraps a spinning mutex, to avoid multiple writes at once.
unsafe impl Sync for StdOutWrapper {}
static STD_OUT: Once<StdOutWrapper> = Once::new();

pub fn set_stdout(std_out: &'static mut dyn Write, min_level: log::LevelFilter) {
    crate::interrupts::without(|| {
        STD_OUT.call_once(|| {
            // SAFETY: We know this will only be called once within this context.
            unsafe { init_logger(Logger::SERIAL, min_level) };

            StdOutWrapper(spin::Mutex::new(std_out))
        })
    });
}
