bitflags::bitflags! {
    pub struct LoggingMode : u8 {
        const SERIAL = 1 << 0;
        const GRAPHIC = 1 << 1;
    }
}

pub struct KernelLogger {
    pub mode: LoggingMode,
}

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Trace
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            if self.mode.contains(LoggingMode::SERIAL) {
                crate::serialln!("[{}] {}", record.level(), record.args());
            }

            if self.mode.contains(LoggingMode::GRAPHIC) {
                panic!("no graphics logging implemented!");
            }
        }
    }

    fn flush(&self) {}
}

const LOGGER: KernelLogger = KernelLogger {
    mode: LoggingMode::SERIAL,
};

pub fn init(min_level: log::LevelFilter) -> Result<(), log::SetLoggerError> {
    if let Err(error) = unsafe { log::set_logger_racy(&LOGGER) } {
        Err(error)
    } else {
        log::set_max_level(min_level);
        Ok(())
    }
}
