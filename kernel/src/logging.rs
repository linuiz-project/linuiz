use log::{Level, Metadata};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            match self.output_mode {
                KernelLogOutputMode::Serial => {
                    crate::serialln!("[{}] {}", record.level(), record.args())
                }
                KernelLogOutputMode::Graphic => panic!("no graphics logging implemented!"),
            }
        }
    }

    fn flush(&self) {}
}

pub const LOGGER: KernelLogger = KernelLogger::new();

#[cfg(debug_assertions)]
fn configure_log_level() {
    log::set_max_level(log::LevelFilter::Debug);
}

#[cfg(not(debug_assertions))]
fn configure_log_level() {
    log::set_max_level(log::LevelFilter::Info);
}

pub unsafe fn init() -> Result<(), log::SetLoggerError> {
    if let Err(error) = log::set_logger_racy(&LOGGER) {
        Err(error)
    } else {
        configure_log_level();
        Ok(())
    }
}
