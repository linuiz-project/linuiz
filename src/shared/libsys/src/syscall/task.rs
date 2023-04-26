use super::{Result, Vector};

pub fn yield_task() -> Result {
    // Safety: We're very careful.
    unsafe {
        let low: u64;
        let high: u64;

        core::arch::asm!(
            "syscall",
            in("rdi") Vector::TaskYield as u64,
            out("rdx") high,
            out("rax") low,
            // caller saved registers
            out("rcx") _,
            out("rsi") _,
            out("r8") _,
            out("r9") _,
            out("r10") _,
            out("r11") _,
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
            "syscall",
            in("rdi") Vector::TaskExit as u64,
            out("rdx") high,
            out("rax") low,
            // caller saved registers
            out("rcx") _,
            out("rsi") _,
            out("r8") _,
            out("r9") _,
            out("r10") _,
            out("r11") _,
            options(nostack, nomem, preserves_flags)
        );

        core::mem::transmute([low, high])
    }
}
