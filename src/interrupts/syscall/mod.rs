use crate::cpu::{context::Context, local_state::LocalState};
use core::num::NonZero;
use libsys::syscall::Vector;

mod klog;
mod task;

#[derive(Debug, Clone, Copy)]
pub struct SyscallResult {
    pub code: Option<NonZero<usize>>,
    pub value: usize,
}

impl SyscallResult {
    pub const fn success() -> Self {
        Self {
            code: None,
            value: 0,
        }
    }

    pub const fn invalid_vector() -> Self {
        Self {
            code: Some(NonZero::<usize>::MAX),
            value: 0,
        }
    }
}

impl<E: core::error::Error + Into<usize>> From<Result<(), E>> for SyscallResult {
    fn from(value: Result<(), E>) -> Self {
        match value {
            Ok(()) => Self {
                code: None,
                value: 0,
            },

            Err(error_code) => {
                let error_code =
                    NonZero::<usize>::new(error_code.into()).expect("syscall error code was 0");

                Self {
                    code: Some(error_code),
                    value: 0,
                }
            }
        }
    }
}

pub fn handle(
    vector: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    context: &mut Context,
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
            LocalState::with_scheduler(|scheduler| scheduler.yield_task(context)).into()
        }

        Ok(Vector::TaskKill) => {
            // LocalState::with_scheduler(|scheduler| scheduler.kill_task(state, regs));
            // SyscallResult::success()

            todo!()
        }
    }
}
