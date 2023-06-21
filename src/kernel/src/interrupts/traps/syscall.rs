use crate::task::{Registers, State};
use libsys::syscall::{Error, Result, Success, Vector};

#[allow(clippy::too_many_arguments)]
pub(super) fn process(
    vector: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
    state: &mut State,
    regs: &mut Registers,
) -> Result {
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

    let result = match Vector::try_from(vector) {
        Err(err) => {
            warn!("Unhandled system call vector: {:X?}", err);
            Err(Error::InvalidVector)
        }

        Ok(Vector::KlogInfo) => process_klog(log::Level::Info, arg0, arg1),
        Ok(Vector::KlogError) => process_klog(log::Level::Error, arg0, arg1),
        Ok(Vector::KlogDebug) => process_klog(log::Level::Debug, arg0, arg1),
        Ok(Vector::KlogTrace) => process_klog(log::Level::Trace, arg0, arg1),

        Ok(Vector::TaskExit) => {
            crate::cpu::state::with_scheduler(|scheduler| scheduler.kill_task(state, regs));

            Ok(Success::Ok)
        }
        Ok(Vector::TaskYield) => {
            crate::cpu::state::with_scheduler(|scheduler| scheduler.yield_task(state, regs));

            Ok(Success::Ok)
        }
    };

    trace!("Syscall: {:X?}", result);

    result
}

fn process_klog(level: log::Level, str_ptr_arg: usize, str_len: usize) -> Result {
    let str_ptr = str_ptr_arg as *mut u8;

    crate::cpu::state::with_scheduler(|scheduler| {
        let task = scheduler.task_mut().ok_or(Error::NoActiveTask)?;
        let address_space = task.address_space_mut();

        let str_start = str_ptr.addr();
        let str_end = str_start + str_len;

        for address in (str_start..str_end).map(libsys::Address::new_truncate) {
            if !address_space.is_mmapped(address) {
                return Err(Error::UnmappedMemory);
            }
        }

        Ok(Success::Ok)
    })?;

    // Safety: TODO
    let str_slice = unsafe { core::slice::from_raw_parts(str_ptr, str_len) };
    let str = core::str::from_utf8(str_slice).map_err(Error::from)?;

    log!(level, "[KLOG]: {}", str);

    Ok(Success::Ok)
}
