bitflags::bitflags! {
    pub struct LoggingModes : u8 {
        const NONE = 0;
        const STDOUT = 1 << 0;
        const GRAPHIC = 1 << 1;
    }
}

static TRACE_ENABLED_PATHS: [&str; 1] = ["libkernel::memory::block_allocator"];

pub struct KernelLogger {
    modes: LoggingModes,
}

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() < log::Level::Trace || TRACE_ENABLED_PATHS.contains(&metadata.target())
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            if self.modes.contains(LoggingModes::STDOUT) {
                crate::println!(
                    "[{} {}] {}",
                    record.level(),
                    record.module_path().unwrap_or("None"),
                    record.args()
                );
            }

            if self.modes.contains(LoggingModes::GRAPHIC) {
                panic!("no graphics logging implemented!");
            }
        }
    }

    fn flush(&self) {}
}

static mut LOGGER: Option<KernelLogger> = None;

pub fn init_logger(
    modes: LoggingModes,
    min_level: log::LevelFilter,
) -> Result<(), log::SetLoggerError> {
    unsafe {
        if LOGGER.is_some() {
            panic!("logger can only be configured once")
        } else {
            LOGGER = Some(KernelLogger { modes });

            match log::set_logger_racy(LOGGER.as_ref().unwrap()) {
                Ok(()) => {
                    log::set_max_level(min_level);
                    Ok(())
                }
                Err(error) => Err(error),
            }
        }
    }
}
