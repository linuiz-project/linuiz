use super::{Result, Vector};

pub fn yield_task() -> Result {
    // Safety: We're very careful.
    unsafe {
        let discriminant: usize;
        let value: usize;

        core::arch::asm!(
            "int 0x80",
            in("rax") Vector::TaskYield as usize,
            out("rdi") discriminant,
            out("rsi") value,
            options(nostack, nomem, preserves_flags)
        );

        <Result as super::ResultConverter>::from_registers((discriminant, value))
    }
}

pub fn exit_task() -> Result {
    // Safety: We're very careful.
    unsafe {
        let discriminant: usize;
        let value: usize;

        core::arch::asm!(
            "int 0x80",
            in("rax") Vector::TaskExit as usize,
            out("rdi") discriminant,
            out("rsi") value,
            options(nostack, nomem, preserves_flags)
        );

        <Result as super::ResultConverter>::from_registers((discriminant, value))
    }
}
