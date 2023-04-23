use super::{Result, Vector};

#[repr(u64)]
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
    let klog_vector = (Vector::KlogInfo as u64) + (offset as u64);
    let str_ptr = str.as_ptr();
    let str_len = str.len();

    // Safety: It isn't.
    unsafe {
        let low: u64;
        let high: u64;

        core::arch::asm!(
            "syscall",
            in("rdi") klog_vector,
            in("rsi") str_ptr,
            inout("rdx") str_len => high,
            out("rax") low,
            // callee saved registers
            out("rcx") _,
            out("r8") _,
            out("r9") _,
            out("r10") _,
            out("r11") _,
            options(nostack, nomem, preserves_flags)
        );

        core::mem::transmute([low, high])
    }
}
