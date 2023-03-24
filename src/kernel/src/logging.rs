use crate::interrupts::InterruptCell;
use spin::Mutex;
use uart::UartWriter;

pub struct Serial(InterruptCell<Mutex<UartWriter>>);

impl log::Log for Serial {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            // TODO tell the time
            let ticks = 0;
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

pub fn init() -> Result<(), log::SetLoggerError> {
    #[cfg(debug_assertions)]
    {
        log::set_max_level(log::LevelFilter::Trace);
    }
    #[cfg(not(debug_assertions))]
    {
        log::set_max_level(log::LevelFilter::Info);
    }

    log::set_logger({
        static SERIAL_UART: spin::Lazy<Serial> = spin::Lazy::new(|| {
            crate::interrupts::without(|| {
                // Safety: Constructor is called only once.
                let uart_writer = unsafe {
                    UartWriter::new(
                        #[cfg(target_arch = "x86_64")]
                        {
                            uart::Uart::<uart::Data>::new(uart::COM1)
                        },
                    )
                };

                Serial(InterruptCell::new(Mutex::new(uart_writer)))
            })
        });

        &*SERIAL_UART
    })
}
