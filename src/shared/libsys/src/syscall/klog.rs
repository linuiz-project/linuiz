use super::{Result, Vector};

enum KlogVectorOffset {
    Info = 0,
    Error = 1,
    Debug = 2,
    Trace = 3,
}

pub fn info(str: &str) -> Result {
    klog(KlogVectorOffset::Info, str)
}

pub fn error(str: &str) -> Result {
    klog(KlogVectorOffset::Error, str)
}

pub fn debug(str: &str) -> Result {
    klog(KlogVectorOffset::Debug, str)
}

pub fn trace(str: &str) -> Result {
    klog(KlogVectorOffset::Trace, str)
}

fn klog(offset: KlogVectorOffset, str: &str) -> Result {
    let vector = (Vector::KlogInfo as usize) + (offset as usize);
    let str_ptr = str.as_ptr();
    let str_len = str.len();

    // Safety: It isn't.
    unsafe {
        let discriminant: usize;
        let value: usize;

        core::arch::asm!(
            "int 0x80",
            in("rax") vector,
            inout("rdi") str_ptr => discriminant,
            inout("rsi") str_len => value,
            options(nostack, preserves_flags)
        );

        <Result as super::ResultConverter>::from_registers((discriminant, value))
    }
}
