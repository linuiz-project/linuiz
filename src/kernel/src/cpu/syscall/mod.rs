#[cfg(target_arch = "x86_64")]
mod x64;
#[cfg(target_arch = "x86_64")]
pub use x64::*;

use crate::task::{Registers, State};
use libsys::syscall::{Result, Vector};

/// ### Safety
///
/// This function should never be called by software.
#[allow(clippy::too_many_arguments)]
pub(self) fn process(
    vector: u64,
    arg0: u64,
    arg1: u64,
    _arg2: u64,
    _arg3: u64,
    _arg4: u64,
    state: &mut State,
    regs: &mut Registers,
) -> Result {
    match Vector::try_from(vector) {
        Err(err) => {
            warn!("Unhandled system call vector: {:X?}", err);
            Result::InvalidVector
        }

        Ok(Vector::KlogInfo) => process_klog(log::Level::Info, arg0, arg1),
        Ok(Vector::KlogError) => process_klog(log::Level::Error, arg0, arg1),
        Ok(Vector::KlogDebug) => process_klog(log::Level::Debug, arg0, arg1),
        Ok(Vector::KlogTrace) => process_klog(log::Level::Trace, arg0, arg1),

        Ok(Vector::TaskExit) => crate::local::with_scheduler(|scheduler| scheduler.exit_task(state, regs)).unwrap(),
        Ok(Vector::TaskYield) => crate::local::with_scheduler(|scheduler| scheduler.yield_task(state, regs)).unwrap(),
    }
}

fn process_klog(level: log::Level, str_ptr_arg: u64, str_len_arg: u64) -> Result {
    let str_ptr = usize::try_from(str_ptr_arg).map_err(Result::from)? as *mut u8;
    let str_len = usize::try_from(str_len_arg).map_err(Result::from)?;

    // Safety: TODO
    let str_slice = unsafe { core::slice::from_raw_parts(str_ptr, str_len) };
    let str = core::str::from_utf8(str_slice).map_err(Result::from)?;
    log!(level, "[KLOG]: {}", str);

    Result::Ok
}
