mod qemu_e9;
mod serial;

pub use qemu_e9::*;
pub use serial::*;

use core::fmt::Write;

struct StdOutCell<'write> {
    stdout: Option<&'write mut dyn Write>,
}

unsafe impl Sync for StdOutCell<'_> {}
unsafe impl Send for StdOutCell<'_> {}

impl<'write> StdOutCell<'write> {
    fn new() -> Self {
        Self { stdout: None }
    }

    fn set(&mut self, stdout: &'write mut dyn Write) {
        self.stdout = Some(stdout);
    }

    fn get_mut(&mut self) -> Option<&mut &'write mut dyn Write> {
        self.stdout.as_mut()
    }
}

lazy_static::lazy_static! {
    static ref STDOUT: spin::Mutex<StdOutCell<'static>> = spin::Mutex::new(StdOutCell::new());
}

pub fn set_stdout(stdout: &'static mut dyn Write) {
    STDOUT.lock().set(stdout);
}

#[doc(hidden)]
pub fn __std_out(args: core::fmt::Arguments) {
    libkernel::instructions::interrupts::without_interrupts(|| {
        let stdout_guard = &mut STDOUT.lock();

        if let Some(stdout) = stdout_guard.get_mut() {
            stdout.write_fmt(args).unwrap();
        } else {
            panic!("sttdout has not been configured");
        }
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::drivers::io::__std_out(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
