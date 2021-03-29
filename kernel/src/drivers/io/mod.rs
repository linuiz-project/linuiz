mod qemu_e9;
mod serial;

pub use qemu_e9::*;
pub use serial::*;

use core::fmt::Write;
use libkernel::cell::SyncRefCell;

static STDOUT: SyncRefCell<&'static mut dyn Write> = SyncRefCell::empty();

pub fn set_stdout(stdout: &'static mut dyn Write) {
    STDOUT.set(stdout);
}

#[doc(hidden)]
pub fn __std_out(args: core::fmt::Arguments) {
    // let stdout_guard = &mut STDOUT.lock();

    if let Some(stdout) = STDOUT.borrow_mut() {
        stdout.write_fmt(args).unwrap();
    } else {
        panic!("stdout has not been configured");
    }
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
