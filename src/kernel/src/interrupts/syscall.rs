use crate::{
    arch::x86_64::structures::idt::InterruptStackFrame, cpu::local_state::LocalState,
    task::Registers,
};
use libsys::syscall::{Error, Result, Success, Vector};

#[allow(clippy::too_many_arguments)]
pub fn process(
    vector: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
    state: &mut InterruptStackFrame,
    regs: &mut Registers,
) -> Result {
    trace!(
        "Syscall Args: Vector:{vector:X?}   0:{arg0:X?}  1:{arg1:X?}  2:{arg2:X?}  3:{arg3:X?}  4:{arg4:X?}  5:{arg5:X?}"
    );

    let result = match Vector::try_from(vector) {
        Err(err) => {
            warn!("Unhandled system call vector: {err:X?}");
            Err(Error::InvalidVector)
        }

        Ok(Vector::KlogInfo) => process_klog(log::Level::Info, arg0, arg1),
        Ok(Vector::KlogError) => process_klog(log::Level::Error, arg0, arg1),
        Ok(Vector::KlogDebug) => process_klog(log::Level::Debug, arg0, arg1),
        Ok(Vector::KlogTrace) => process_klog(log::Level::Trace, arg0, arg1),

        Ok(Vector::TaskExit) => {
            LocalState::with_scheduler(|scheduler| scheduler.kill_task(state, regs));

            Ok(Success::Ok)
        }
        Ok(Vector::TaskYield) => {
            LocalState::with_scheduler(|scheduler| scheduler.yield_task(state, regs));

            Ok(Success::Ok)
        }
    };

    trace!("Syscall Result: {result:X?}");

    result
}

fn process_klog(level: log::Level, str_ptr_arg: usize, str_len: usize) -> Result {
    let str_ptr = core::ptr::with_exposed_provenance::<u8>(str_ptr_arg);

    // TODO abstract this into a function
    LocalState::with_scheduler(|scheduler| {
        use crate::task::Error as TaskError;
        use libsys::{Address, page_size};

        let str_start = str_ptr.addr();
        let str_end = str_start + str_len;

        let task = scheduler.task_mut().ok_or(Error::NoActiveTask)?;
        for address in (str_start..str_end)
            .step_by(page_size())
            .map(Address::new_truncate)
        {
            match task.demand_map(address) {
                Ok(()) | Err(TaskError::AlreadyMapped) => {}

                err => {
                    warn!("Failed to demand map: {err:X?}");
                    return Err(Error::UnmappedMemory);
                }
            }
        }

        Ok(Success::Ok)
    })?;

    // Safety: TODO
    let str_slice = unsafe { core::slice::from_raw_parts(str_ptr, str_len) };
    let str = core::str::from_utf8(str_slice).map_err(Error::from)?;

    log!(level, "[KLOG]: {str}");

    Ok(Success::Ok)
}
