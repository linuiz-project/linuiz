use core::arch::asm;

extern "C" {
    static __syscall_stack: lib::LinkerSymbol;
}

static SYSCALL_FUNCTIONS: [extern "sysv64" fn(); 1] = [worked];

pub enum SyscallID {
    Worked,
}

/// Syscalls use the x64 SysV ABI, with these caveats:
///     - Register `r10` contains the sycall ID
///     - Register `rcx` is reserved by the function.
#[naked]
#[no_mangle]
#[allow(named_asm_labels)]
pub(crate) unsafe extern "C" fn syscall_entry() {
    asm!(
    "
        /* 
         * Parameter registers:
         * rdi, rsi, rdx, r8, r9
         * 
         * Preserved registers:
         * rbx, rcx, rsp, rbp, r12, r13, r14, r15
         * 
         * Unused (in-function) registers:
         * rax
         * 
         * Return registers (shares with preserved registers):
         * rax, rbx, rdx, rsp, rbp, r12, r13, r14, r15
         * 
         * Syscall registers:
         * r10, (preserve/restore:) rcx, r11
         */

        /* syscall preserve */
        push rcx
        push r11

        /* Syscall stack */
        push rsp
        lea rsp, {0}

        rep:
            jmp rep

        /* SysV64 preserve */
        push rbx
        push rsp
        push rbp
        push r12
        push r13
        push r14
        push r15

        mov rax, 0x8    /* 8 byte absolute index */
        mul r10         /* Absolute index offset in SYSCALL_FUNCTIONS */
        mov rbx, {1}
        add rbx, r10    /* Absolute address of syscall function */

        call rbx

        /* SysV64 restore */
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rsp
        pop rbx
        
        /* Return stack */
        pop rsp

        /* syscall restore */
        pop r11
        pop rcx
        sysret
    ",
    sym __syscall_stack,
    sym SYSCALL_FUNCTIONS,
    options(noreturn),
    );
}

extern "sysv64" fn worked() {
    info!("worked");
}
