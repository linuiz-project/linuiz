bitflags::bitflags! {
    pub struct LoggingModes : u8 {
        const NONE = 0;
        const SERIAL = 1 << 0;
        const GRAPHIC = 1 << 1;
    }
}

pub struct KernelLogger {
    pub modes: LoggingModes,
}

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Trace
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            if self.modes.contains(LoggingModes::SERIAL) {
                crate::serialln!("[{}] {}", record.level(), record.args());
            }

            if self.modes.contains(LoggingModes::GRAPHIC) {
                panic!("no graphics logging implemented!");
            }
        }
    }

    fn flush(&self) {}
}

static mut LOGGER: KernelLogger = KernelLogger {
    modes: LoggingModes::NONE,
};

pub fn init(modes: LoggingModes, min_level: log::LevelFilter) -> Result<(), log::SetLoggerError> {
    unsafe { LOGGER = KernelLogger { modes } };

    if let Err(error) = unsafe { log::set_logger_racy(&LOGGER) } {
        Err(error)
    } else {
        log::set_max_level(min_level);
        Ok(())
    }
}
