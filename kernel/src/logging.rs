pub struct LogMessage {
    cpu: u32,
    timestamp: u64,
    level: log::Level,
    body: alloc::string::String,
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct LoggingModes : u8 {
        const NONE = 0;
        const SERIAL = 1 << 0;
        const GRAPHIC = 1 << 1;
    }
}

pub struct KernelLogger {
    modes: LoggingModes,
}

impl log::Log for KernelLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let ticks = crate::clock::get_ticks();
            let whole_time = ticks / 1000;
            let frac_time = ticks % 1000;

            // TODO possibly only log CPU# in debug builds (or debug/trace messages?)
            crate::println!(
                "[{whole_time:wwidth$}.{frac_time:0fwidth$}][CPU{cpu_id}][{level}] {args}",
                cpu_id = libarch::cpu::get_id(),
                level = record.level(),
                args = record.args(),
                wwidth = 4,
                fwidth = 3
            );
        }
    }

    fn flush(&self) {}
}

static mut LOGGER: Option<KernelLogger> = None;

pub fn init_logger(modes: LoggingModes, min_level: log::LevelFilter) -> Result<(), log::SetLoggerError> {
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
