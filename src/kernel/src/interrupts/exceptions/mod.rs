mod page_fault;

mod arch;
pub use arch::*;

use core::ptr::NonNull;

#[doc(hidden)]
#[inline(never)]
pub fn handle(exception: &ArchException) {
    match exception {
        // Safety: Function is called once per this page fault exception.
        ArchException::PageFault(_, _, _, address) => unsafe {
            if let Err(err) = page_fault::handler(*address) {
                panic!("error handling page fault: {}", err)
            }
        },

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
        ptr: NonNull<u8>,
        reason: PageFaultReason,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Exception {
    kind: ExceptionKind,
    ip: NonNull<u8>,
    sp: NonNull<u8>,
}

impl Exception {
    pub const fn new(kind: ExceptionKind, ip: NonNull<u8>, sp: NonNull<u8>) -> Self {
        Self { kind, ip, sp }
    }
}
