use crate::task::{Registers, State};
use libsys::syscall::{Result, Vector};

#[allow(clippy::too_many_arguments)]
pub(super) fn process(
    vector: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    state: &mut State,
    regs: &mut Registers,
) -> Option<Result> {
    trace!(
        "Syscall Args: Vector:{:X?}   0:{:X?}  1:{:X?}  2:{:X?}  3:{:X?}  4:{:X?}  5:{:X?}",
        vector,
        arg0,
        arg1,
        arg2,
        arg3,
        arg4,
        arg5
    );

    match Vector::try_from(vector) {
        Err(err) => {
            warn!("Unhandled system call vector: {:X?}", err);
            Some(Result::InvalidVector)
        }

        Ok(Vector::KlogInfo) => Some(process_klog(log::Level::Info, arg0, arg1)),
        Ok(Vector::KlogError) => Some(process_klog(log::Level::Error, arg0, arg1)),
        Ok(Vector::KlogDebug) => Some(process_klog(log::Level::Debug, arg0, arg1)),
        Ok(Vector::KlogTrace) => Some(process_klog(log::Level::Trace, arg0, arg1)),

        Ok(Vector::TaskExit) => {
            crate::cpu::state::with_scheduler(|scheduler| scheduler.kill_task(state, regs)).unwrap();
            None
        }
        Ok(Vector::TaskYield) => {
            crate::cpu::state::with_scheduler(|scheduler| scheduler.yield_task(state, regs)).unwrap();
            None
        }
    }
}

fn process_klog(level: log::Level, str_ptr_arg: u64, str_len_arg: u64) -> Result {
    let str_ptr = usize::try_from(str_ptr_arg).unwrap() as *mut u8;
    let str_len = usize::try_from(str_len_arg).unwrap();

    // Safety: TODO
    let str_slice = unsafe { core::slice::from_raw_parts(str_ptr, str_len) };
    let str = core::str::from_utf8(str_slice).map_err(Result::from)?;
    log!(level, "[KLOG]: {}", str);

    Result::Ok
}
