use crate::interrupts::InterruptCell;
use core::fmt::Write;
use spin::{Mutex, Once};

mod buffered_uart;

static UART: Once<InterruptCell<Mutex<buffered_uart::BufferedUart>>> = Once::new();

struct BufferedUartLogger;

// Safety: Interior address is not thread-specific.
unsafe impl Send for BufferedUartLogger {}
// Safety: `Serial` constructor requires caller to ensure only one instance exists per port.
unsafe impl Sync for BufferedUartLogger {}

impl core::fmt::Write for BufferedUartLogger {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let uart = UART.get().expect("UART not configured");

        let mut finished_write = false;

        while !finished_write {
            uart.with(|uart| {
                let mut uart = uart.lock();

                match uart.buffer_data(s.as_bytes()) {
                    Ok(()) => finished_write = true,
                    Err(buffered_uart::Error::TxBufferFull) => {}
                }
            });
        }

        Ok(())
    }
}

impl log::Log for BufferedUartLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            // TODO tell the time?
            let ticks = 1;
            let whole_time = ticks / 1000;
            let frac_time = ticks % 1000;

            writeln!(
                BufferedUartLogger,
                "[{whole_time:wwidth$}.{frac_time:0fwidth$}][{level}] {args}",
                level = record.level(),
                args = record.args(),
                wwidth = 4,
                fwidth = 3
            )
            .unwrap();
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    #[cfg(debug_assertions)]
    log::set_max_level(log::LevelFilter::Trace);
    #[cfg(not(debug_assertions))]
    log::set_max_level(log::LevelFilter::Trace);

    #[cfg(target_arch = "x86_64")]
    let uart_address = uart::COM1;

    UART.call_once(|| {
        // Safety: This is the only `Serial` that will be constructed.
        unsafe { buffered_uart::BufferedUart::new(uart_address) }
            .map(Mutex::new)
            .map(InterruptCell::new)
            .expect("failed to initialize serial UART")
    });

    log::set_logger(&BufferedUartLogger).unwrap();
}
