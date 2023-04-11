use crate::interrupts::InterruptCell;
use spin::Mutex;
use uart::{Data, Uart, UartWriter};

pub struct Serial(InterruptCell<Mutex<UartWriter>>);

// Safety: Interior address is not thread-specific.
unsafe impl Send for Serial {}
// Safety: This isn't actually safe. It relies entirely on only
//         one `Serial` being created and used at a time.
//         So basically, TODO.
unsafe impl Sync for Serial {}

impl log::Log for Serial {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            // TODO tell the time
            let ticks = 1;
            let whole_time = ticks / 1000;
            let frac_time = ticks % 1000;
            self.0.with(|uart| {
                use core::fmt::Write;

                let mut uart = uart.lock();

                uart.write_fmt(format_args!(
                    "[{whole_time:wwidth$}.{frac_time:0fwidth$}][{level}] {args}\n",
                    level = record.level(),
                    args = record.args(),
                    wwidth = 4,
                    fwidth = 3
                ))
                .unwrap();
            });
        }
    }

    fn flush(&self) {}
}

#[derive(Debug)]
pub enum Error {
    SetLoggerError,
    NoLoggerError,
}

impl core::error::Error for Error {}

impl From<log::SetLoggerError> for Error {
    fn from(_: log::SetLoggerError) -> Self {
        Self::SetLoggerError
    }
}

crate::default_display_impl!(Error);

pub fn init() -> Result<(), Error> {
    #[cfg(debug_assertions)]
    {
        log::set_max_level(log::LevelFilter::Trace);
    }
    #[cfg(not(debug_assertions))]
    {
        log::set_max_level(log::LevelFilter::Info);
    }

    static SERIAL_UART: spin::Lazy<Option<Serial>> = spin::Lazy::new(|| {
        crate::interrupts::without(|| {
            UartWriter::new(
                #[cfg(target_arch = "x86_64")]
                // Safety: Constructor is called only once, with a hopefully-valid address.
                unsafe {
                    Uart::<Data>::new(uart::COM1)
                },
            )
            .map(Mutex::new)
            .map(InterruptCell::new)
            .map(Serial)
        })
    });

    SERIAL_UART.as_ref().ok_or(Error::NoLoggerError).and_then(|serial| log::set_logger(serial).map_err(Error::from))
}
