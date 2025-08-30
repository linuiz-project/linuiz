use crate::{interrupts::uninterruptable, util::sync::Once};

mod uart;

/// The kernel logger.
pub struct KernelLogger {
    serial: &'static uart::Logger,
}

impl KernelLogger {
    pub fn init() {
        uninterruptable(|| {
            static LOGGER: Once<KernelLogger> = Once::new();

            let kernel_logger = LOGGER.call_once(|| Self {
                serial: uart::Logger::init(),
            });

            log::set_max_level(log::LevelFilter::Trace);
            log::set_logger(kernel_logger).unwrap();
        });
    }
}

impl log::Log for KernelLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        unimplemented!()
    }

    fn log(&self, record: &log::Record) {
        self.serial.log(record);
    }

    fn flush(&self) {
        unimplemented!()
    }
}

fn with_formatted_log_record(record: &log::Record, func: impl FnOnce(core::fmt::Arguments)) {
    func(format_args!(
        "[#{hwthread_id}][{level}][{target}] {args}\n",
        hwthread_id = crate::cpu::get_id(),
        level = record.level(),
        target = record.target(),
        args = record.args(),
    ));
}
