use core::ptr::NonNull;

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
    pub fn new(kind: ExceptionKind, ip: NonNull<u8>, sp: NonNull<u8>) -> Self {
        Self { kind, ip, sp }
    }
}
