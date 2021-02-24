pub mod emuout;
pub mod serial;

use core::fmt::Write;
use core::lazy::OnceCell;

struct StdOutCell<'stdout> {
    stdout: Option<&'stdout mut dyn Write>,
}

unsafe impl Sync for StdOutCell<'_> {}

impl<'stdout> StdOutCell<'stdout> {
    fn new() -> Self {
        Self { stdout: None }
    }

    fn set(&mut self, stdout: &'stdout mut dyn Write) {
        self.stdout = Some(stdout);
    }

    fn get_mut(&mut self) -> Option<&'stdout mut dyn Write> {
        self.stdout
    }
}

static STDOUT: spin::Mutex<StdOutCell> = spin::Mutex::new(StdOutCell::new());

pub fn set_stdout(stdout: &'static mut dyn Write) {
    STDOUT.lock().set(stdout);
}

#[doc(hidden)]
pub fn __std_out(args: core::fmt::Arguments) {
    libkernel::instructions::interrupts::without_interrupts(|| {
        let stdout_guard = &mut STDOUT.lock();

        if let Some(stdout) = stdout_guard.get_mut() {
            stdout.write_fmt(args);
        } else {
            panic!("sttdout has not been configured");
        }
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::drivers::io::serial::__std_out(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
