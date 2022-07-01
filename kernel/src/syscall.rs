#![allow(named_asm_labels)]

use crate::scheduling::ThreadRegisters;
use core::arch::asm;
use x86_64::structures::idt::InterruptStackFrame;

extern "C" {
    // TODO syscall stack per-cpu
    static __syscall_stack: libkernel::LinkerSymbol;
}

static mut SYSCALL_FUNCTIONS: [unsafe extern "win64" fn(
    &mut InterruptStackFrame,
    *mut ThreadRegisters,
); 1] = [syscall_test];

#[repr(u64)]
pub enum SyscallID {
    Test,
}

#[naked]
pub(crate) unsafe extern "C" fn syscall_enter() {
    asm!(
        "
        /* Move to syscall stack. */
        mov r12, rsp

        swapgs
        
        mov rsp, gs:{}

        /* Push fake stack frame for scheduling compatibility. */
        push 0x0 /* Push empty stack seg value. */
        push r12 /* Push `rsp` value. */
        push r11 /* Push `rflags` value. */
        push 0x0 /* Push empty code seg value. */
        push rcx /* Push `rip` value. */

        /* Push all gprs to the stack. */
        push r15
        push r14
        push r13
        push r12 /* sysret stack ptr */
        push r11 /* sysret rflags */
        push r10 /* syscall id */
        push r9
        push r8
        push rbp
        push rdi
        push rsi
        push rdx /* segment loader */
        push rcx /* sysret rip */
        push rbx
        push rax

        /* Save location of registers. */
        mov rcx, rsp
        add rcx, 15 * 8 /* ISF will be just before the 14 registers we pushed. */
        /* Move cached gprs pointer into second parameter. */
        mov rdx, rsp

        cld

        /* Load the function pointer. */
        lea r10, [r10 * 8]
        lea rbx, [r10 + {}]

        call [rbx]

        /* Restore general purpose registers. */
        pop rax
        pop rbx
        pop rcx
        pop rdx
        pop rsi
        pop rdi
        pop rbp
        pop r8
        pop r9
        pop r10
        pop r11
        pop r12
        pop r13
        pop r14
        pop r15

        /* Pop fake stack frame. */
        pop rcx /* Pop cached `rip` value. */
        add rsp, 0x8 /* Pop empty code seg value. */
        pop r11 /* Pop cached `rflags` value. */
        pop r12 /* Pop cached `rsp` value. */
        add rsp, 0x8 /* Pop empty stack seg value. */

        swapgs
        
        /* Restore previous stack. */
        mov rsp, r12

        sysretq
        ",
        const crate::local_state::Offset::SyscallStackPtr as u64,
        sym SYSCALL_FUNCTIONS,
        options(noreturn),
    );
}

unsafe extern "win64" fn syscall_test(
    stack_frame: &mut InterruptStackFrame,
    cached_regs: *mut ThreadRegisters,
) {
    info!("{:#?}\n{:?}", stack_frame, cached_regs.read_volatile());
}
