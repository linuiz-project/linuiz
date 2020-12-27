#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelLoggingMode {
    Serial,
    Graphic,
}

pub struct KernelLogger {
    pub mode: KernelLoggingMode,
}

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Trace
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            match self.mode {
                KernelLoggingMode::Serial => {
                    crate::serialln!("[{}] {}", record.level(), record.args())
                }
                KernelLoggingMode::Graphic => panic!("no graphics logging implemented!"),
            }
        }
    }

    fn flush(&self) {}
}

const LOGGER: KernelLogger = KernelLogger {
    mode: KernelLoggingMode::Serial,
};

pub fn init() -> Result<(), log::SetLoggerError> {
    #[cfg(debug_assertions)]
    fn configure_log_level() {
        log::set_max_level(log::LevelFilter::Debug);
    }

    #[cfg(not(debug_assertions))]
    fn configure_log_level() {
        log::set_max_level(log::LevelFilter::Debug);
    }

    if let Err(error) = unsafe { log::set_logger_racy(&LOGGER) } {
        Err(error)
    } else {
        configure_log_level();
        Ok(())
    }
}
