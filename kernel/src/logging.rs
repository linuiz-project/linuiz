pub struct LogMessage {
    cpu: u32,
    timestamp: u64,
    level: log::Level,
    body: alloc::string::String,
}

static LOG_MESSAGES_ENABLED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
static LOG_MESSAGES: crossbeam_queue::SegQueue<LogMessage> = crossbeam_queue::SegQueue::new();

pub fn flush_log_messages_indefinite() -> ! {
    LOG_MESSAGES_ENABLED.store(true, core::sync::atomic::Ordering::Relaxed);

    loop {
        if let Some(log_message) = LOG_MESSAGES.pop() {
            let whole_time = log_message.timestamp / 1000;
            let frac_time = log_message.timestamp % 1000;

            // TODO possibly only log CPU# in debug builds
            crate::println!(
                "[{:wwidth$}.{:0fwidth$}][CPU{}][{}] {}",
                whole_time,
                frac_time,
                log_message.cpu,
                log_message.level,
                log_message.body,
                wwidth = 4,
                fwidth = 3
            );
        }

        // TODO this shouldn't be a busy wait, probably
        crate::clock::busy_wait_msec(1);
    }
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
            if LOG_MESSAGES_ENABLED.load(core::sync::atomic::Ordering::Relaxed) {
                LOG_MESSAGES.push(LogMessage {
                    cpu: libkernel::structures::apic::get_id(),
                    timestamp: crate::clock::get_ticks(),
                    level: record.level(),
                    body: alloc::format!("{}", record.args()),
                });
            } else {
                let ticks = crate::clock::get_ticks();
                let whole_time = ticks / 1000;
                let frac_time = ticks % 1000;

                // TODO possibly only log CPU# in debug builds
                crate::println!(
                    "[{:wwidth$}.{:0fwidth$}][CPU{}][{}] {}",
                    whole_time,
                    frac_time,
                    libkernel::structures::apic::get_id(),
                    record.level(),
                    record.args(),
                    wwidth = 4,
                    fwidth = 3
                );
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
