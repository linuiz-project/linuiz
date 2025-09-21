#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use self::x86_64::*;

#[derive(Debug, Clone, Copy)]
pub enum PageFaultReason {
    BadPermissions,
    NotMapped,
}

#[doc(hidden)]
#[inline(never)]
#[allow(clippy::needless_pass_by_value)]
pub fn handle(exception: ArchException) {
    debug!("{exception:#X?}");

    match exception {
        ArchException::Breakpoint(_, _) => {
            // TODO Handle sending breakpoints to attached processes.
        }

        // Safety: Function is called once per this page fault exception.
        ArchException::PageFault(_, _, error_code, address) => {
            panic!("page fault: {error_code:?} @ {address:?}")
        }

        _ => panic!("could not handle exception"),
    }
}
