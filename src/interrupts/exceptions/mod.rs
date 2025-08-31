use core::ptr::NonNull;
use libsys::address::{Address, Virtual};

mod arch;
pub use arch::*;

#[doc(hidden)]
#[inline(never)]
pub fn handle(exception: ArchException) {
    match exception {
        // Safety: Function is called once per this page fault exception.
        ArchException::PageFault(_, _, error_code, address) => {
            panic!("page fault: {error_code:?} @ {address:?}")
        }

        exception => panic!("{exception:#X?}"),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PageFaultReason {
    BadPermissions,
    NotMapped,
}

#[derive(Debug, Clone, Copy)]
pub enum ExceptionKind {
    PageFault {
        address: Address<Virtual>,
        cause: PageFaultReason,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Exception {
    kind: ExceptionKind,
    ip: Option<NonNull<u8>>,
    sp: Option<NonNull<u8>>,
}

impl Exception {
    pub const fn new(
        kind: ExceptionKind,
        ip: Option<NonNull<u8>>,
        sp: Option<NonNull<u8>>,
    ) -> Self {
        Self { kind, ip, sp }
    }
}
