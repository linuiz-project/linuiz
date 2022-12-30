use core::ptr::NonNull;

pub enum PageFaultReason {
    BadPermissions,
    InvalidPtr,
}

pub enum ExceptionKind {
    PageFault { ptr: NonNull<u8>, reason: PageFaultReason },
}

pub struct Exception {
    kind: ExceptionKind,
    ip: NonNull<u8>,
    sp: NonNull<u8>,
}

impl Exception {
    pub fn new(kind: ExceptionKind, ip: NonNull<u8>, sp: NonNull<u8>) -> Self {
        Self { kind, ip, sp }
    }
}
