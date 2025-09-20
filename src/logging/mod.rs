use crate::{interrupts::uninterruptable, util::sync::Once};

mod debug;
mod serial;

/// The kernel logger.
pub struct KernelLogger {
    debug: Option<&'static debug::Logger>,
    serial: &'static serial::Logger,
}

impl KernelLogger {
    pub fn init() {
        uninterruptable(|| {
            static LOGGER: Once<KernelLogger> = Once::new();

            let kernel_logger = LOGGER.call_once(|| Self {
                debug: debug::Logger::init(),
                serial: serial::Logger::init(),
            });

            log::set_max_level(log::LevelFilter::Trace);
            log::set_logger(kernel_logger).unwrap();
        });
    }
}

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Trace
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        self.debug.inspect(|logger| logger.log(record));
        self.serial.log(record);
    }

    fn flush(&self) {
        unimplemented!()
    }
}

fn with_formatted_log_record(record: &log::Record, func: impl FnOnce(core::fmt::Arguments)) {
    func(format_args!(
        "[CPU#{processor_id}][{level}][{target}] {args}\n",
        processor_id = crate::cpu::get_id(),
        level = record.level(),
        target = record.target(),
        args = record.args(),
    ));
}
