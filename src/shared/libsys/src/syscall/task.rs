use super::{Result, Vector};

pub fn yield_task() -> Result {
    // Safety: We're very careful.
    unsafe {
        let low: u64;
        let high: u64;

        core::arch::asm!(
            "int 0x80",
            in("rax") Vector::TaskYield as u64,
            out("rdi") low,
            out("rsi") high,
            options(nostack, nomem, preserves_flags)
        );

        core::mem::transmute([low, high])
    }
}

pub fn exit_task() -> Result {
    // Safety: We're very careful.
    unsafe {
        let low: u64;
        let high: u64;

        core::arch::asm!(
            "int 0x80",
            in("rax") Vector::TaskExit as u64,
            out("rdi") low,
            out("rsi") high,
            options(nostack, nomem, preserves_flags)
        );

        core::mem::transmute([low, high])
    }
}
