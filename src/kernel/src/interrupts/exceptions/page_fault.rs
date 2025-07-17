use libsys::{Address, Virtual};

use crate::cpu::local_state::LocalState;

/// Indicates what type of error the common page fault handler encountered.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    #[error("there's no active task")]
    NoTask,

    #[error("failed to deman map memory")]
    Task(#[from] crate::task::Error),
}

/// ## Safety
///
/// This function should only be called in the case of passing context to handle a page fault.
/// Calling this function more than once and/or outside the context of a page fault is undefined behaviour.
#[doc(hidden)]
#[inline(never)]
pub unsafe fn handler(fault_address: Address<Virtual>) -> Result<(), Error> {
    LocalState::with_scheduler(|scheduler| {
        scheduler
            .task_mut()
            .ok_or(Error::NoTask)?
            .demand_map(fault_address)?;

        Ok::<(), Error>(())
    })?;

    Ok(())
}
