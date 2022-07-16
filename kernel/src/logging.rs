bitflags::bitflags! {
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
            if self.modes.contains(LoggingModes::SERIAL) {
                let ticks = crate::clock::global::get_ticks();
                let whole_time = ticks / 1000;
                let frac_time = ticks % 1000;

                crate::println!(
                    "[{:wwidth$}.{:0fwidth$}][CPU{}][{} {}] {}",
                    whole_time,
                    frac_time,
                    liblz::structures::apic::get_id(),
                    record.level(),
                    record.module_path().unwrap_or("*"),
                    record.args(),
                    wwidth = 4,
                    fwidth = 4
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
