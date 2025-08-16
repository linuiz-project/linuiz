use crate::{
    arch::x86_64::structures::idt::InterruptStackFrame, cpu::local_state::LocalState,
    task::Registers,
};
use core::num::NonZero;
use libsys::syscall::Vector;

mod klog;
mod task;

#[derive(Debug)]
pub struct SyscallResult {
    pub code: Option<NonZero<usize>>,
    pub value: usize,
}

impl SyscallResult {
    pub fn success() -> Self {
        Self {
            code: None,
            value: 0,
        }
    }

    pub fn invalid_vector() -> Self {
        Self {
            code: Some({
                // Safety: Value is non-zero.
                unsafe { NonZero::new_unchecked(usize::MAX) }
            }),
            value: 0,
        }
    }
}

impl<TError: core::error::Error + Into<usize>> From<Result<(), TError>> for SyscallResult {
    fn from(value: Result<(), TError>) -> Self {
        match value {
            Ok(()) => Self {
                code: None,
                value: 0,
            },

            Err(error) => {
                let error_code =
                    NonZero::<usize>::new(error.into()).expect("syscall error code was 0");

                Self {
                    code: Some(error_code),
                    value: 0,
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn process(
    vector: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    state: &mut InterruptStackFrame,
    regs: &mut Registers,
) -> SyscallResult {
    trace!(
        "Syscall: {{ Vector: {vector:#X}, 1: {arg1:#X}, 2: {arg2:#X}, 3: {arg3:#X}, 4: {arg4:#X}  4:{arg4:X?}"
    );

    match Vector::try_from(vector) {
        Err(err) => {
            warn!("Unhandled system call vector: {err:X?}");

            SyscallResult::invalid_vector()
        }

        Ok(Vector::KlogTrace) => klog::process_klog(log::Level::Trace, arg1, arg2).into(),
        Ok(Vector::KlogDebug) => klog::process_klog(log::Level::Debug, arg1, arg2).into(),
        Ok(Vector::KlogInfo) => klog::process_klog(log::Level::Info, arg1, arg2).into(),
        Ok(Vector::KlogWarn) => klog::process_klog(log::Level::Warn, arg1, arg2).into(),
        Ok(Vector::KlogError) => klog::process_klog(log::Level::Error, arg1, arg2).into(),

        Ok(Vector::TaskDefer) => {
            LocalState::with_scheduler(|scheduler| scheduler.yield_task(state, regs));

            SyscallResult::success()
        }
        Ok(Vector::TaskKill) => {
            LocalState::with_scheduler(|scheduler| scheduler.kill_task(state, regs));

            SyscallResult::success()
        }
    }
}
