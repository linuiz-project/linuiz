use core::ptr::NonNull;

mod page_fault;

mod arch;
pub use arch::*;

#[doc(hidden)]
#[inline(never)]
pub fn handle(exception: &ArchException) {
    trace!("Exception:\n{exception:#X?}");

    match exception {
        // Safety: Function is called once per this page fault exception.
        ArchException::PageFault(_, _, _, address) => unsafe {
            if let Err(err) = page_fault::handler(*address) {
                panic!("error handling page fault: {}", err)
            }
        },

        _ => panic!("could not handle exception!"),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PageFaultReason {
    BadPermissions,
    NotMapped,
}

#[derive(Debug, Clone, Copy)]
pub enum ExceptionKind {
    PageFault { ptr: NonNull<u8>, reason: PageFaultReason },
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
