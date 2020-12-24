use log::{Level, Metadata};

use crate::serialln;

pub enum KernelLogOutputMode {
    Serial,
    Graphic,
}

pub struct KernelLogger {
    pub output_mode: KernelLogOutputMode,
}

impl KernelLogger {
    const fn new() -> Self {
        Self {
            output_mode: KernelLogOutputMode::Serial,
        }
    }
}

impl log::Log for KernelLogger {
    #[cfg(debug_assertions)]
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    #[cfg(not(debug_assertions))]
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            match self.output_mode {
                KernelLogOutputMode::Serial => serialln!("[{}] {}", record.level(), record.args()),
                KernelLogOutputMode::Graphic => panic!("no graphics logging implemented!"),
            }
        }
    }

    fn flush(&self) {}
}

pub const LOGGER: KernelLogger = KernelLogger::new();

#[cfg(debug_assertions)]
fn configure_log_level() {
    use log::{set_max_level, LevelFilter};
    set_max_level(LevelFilter::Debug);
}

#[cfg(not(debug_assertions))]
fn configure_log_level() {
    use log::{set_max_level, LevelFilter};
    set_max_level(LevelFilter::Info);
}

pub unsafe fn init() -> Result<(), log::SetLoggerError> {
    match log::set_logger_racy(&LOGGER) {
        Ok(()) => {
            configure_log_level();
            Ok(())
        }
        Err(error) => Err(error),
    }
}
