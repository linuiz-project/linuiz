mod serial;

#[cfg(debug_assertions)]
mod debug;

/// The kernel logger.
pub struct Logger {
    serial: Option<&'static serial::Logger>,

    #[cfg(debug_assertions)]
    debug: &'static debug::Logger,
}

impl Logger {
    pub fn init() {
        crate::interrupts::uninterruptable(|| {
            static LOGGER: spin::Once<Logger> = spin::Once::new();

            let static_logger = LOGGER.call_once(|| Self {
                serial: serial::Logger::init().ok(),

                #[cfg(debug_assertions)]
                debug: debug::Logger::init(),
            });

            log::set_max_level(log::LevelFilter::Trace);
            log::set_logger(static_logger).unwrap();
        });
    }
}

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        unimplemented!()
    }

    fn log(&self, record: &log::Record) {
        #[cfg(debug_assertions)]
        self.debug.log(record);

        if let Some(serial_logger) = self.serial {
            serial_logger.log(record);
        }
    }

    fn flush(&self) {
        unimplemented!()
    }
}

fn with_formatted_log_record(record: &log::Record, func: impl FnOnce(core::fmt::Arguments)) {
    func(format_args!(
        "[#{hwthread_id}][{level}][{target}] {args}\n",
        hwthread_id = crate::cpu::get_id(),
        level = record.level(),
        target = record.target(),
        args = record.args(),
    ));
}
