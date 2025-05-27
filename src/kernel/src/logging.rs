use core::fmt::Write;

use crate::interrupts::InterruptCell;
use spin::Mutex;
use uart::{UartAddress, writer::UartWriter};

pub struct Serial(InterruptCell<Mutex<UartWriter>>);

// Safety: Interior address is not thread-specific.
unsafe impl Send for Serial {}
// safety: So long as only one `Serial` exists for each port, this is invariant is kept.
unsafe impl Sync for Serial {}

impl Serial {
    /// ## Safety
    ///
    /// - `address` must be a valid serial address pointing to a UART 16550 device.
    /// - `address` must not be read from or written to by another context.
    pub unsafe fn new(address: UartAddress) -> Option<Self> {
        crate::interrupts::without(|| {
            #[cfg(target_arch = "x86_64")]
            // Safety: Constructor is called only once, and address is invariantly valid.
            let uart_writer = unsafe { UartWriter::new(address) };

            uart_writer.map(Mutex::new).map(InterruptCell::new).map(Self)
        })
    }
}

impl log::Log for Serial {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            // TODO tell the time?
            let ticks = 1;
            let whole_time = ticks / 1000;
            let frac_time = ticks % 1000;

            self.0.with(|mutex| {
                let mut uart_writer = mutex.lock();
                writeln!(
                    &mut uart_writer,
                    "[{whole_time:wwidth$}.{frac_time:0fwidth$}][{level}] {args}",
                    level = record.level(),
                    args = record.args(),
                    wwidth = 4,
                    fwidth = 3
                )
                .unwrap();
            });
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    #[cfg(debug_assertions)]
    log::set_max_level(log::LevelFilter::Trace);
    #[cfg(not(debug_assertions))]
    log::set_max_level(log::LevelFilter::Trace);

    static SERIAL_UART: spin::Lazy<Option<Serial>> = spin::Lazy::new(|| {
        crate::interrupts::without(|| {
            // Safety: Provide port *should* be a valid target systems.
            unsafe {
                Serial::new({
                    #[cfg(target_arch = "x86_64")]
                    {
                        uart::COM1
                    }
                })
            }
        })
    });

    let uart = SERIAL_UART.as_ref().unwrap();
    log::set_logger(uart).unwrap();
}
