use libsys::address::{Address, Virtual};

mod page_fault;

mod arch;
pub use arch::*;

#[doc(hidden)]
#[inline(never)]
pub fn handle(exception: &ArchException) {
    match exception {
        // Safety: Function is called once per this page fault exception.
        ArchException::PageFault(_, _, _, address) => unsafe {
            if let Err(err) = page_fault::handler(*address) {
                panic!("error handling page fault: {err}")
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
        address: Address<Virtual>,
        cause: PageFaultReason,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Exception {
    kind: ExceptionKind,
    ip: Address<Virtual>,
    sp: Address<Virtual>,
}

impl Exception {
    pub const fn new(kind: ExceptionKind, ip: Address<Virtual>, sp: Address<Virtual>) -> Self {
        Self { kind, ip, sp }
    }
}
