#![allow(named_asm_labels)]

use crate::scheduling::ThreadRegisters;
use core::arch::asm;
use x86_64::structures::idt::InterruptStackFrame;

extern "C" {
    // TODO syscall stack per-cpu
    static __syscall_stack: lib::LinkerSymbol;
}

#[export_name = "__syscall_functions"]
static mut SYSCALL_FUNCTIONS: [unsafe extern "win64" fn(
    &mut InterruptStackFrame,
    *mut ThreadRegisters,
); 1] = [syscall_test];

#[repr(u64)]
pub enum SyscallID {
    Test,
}

#[naked]
#[no_mangle]
pub(crate) unsafe extern "C" fn syscall_enter() {
    asm!(
        "
        /* Move to syscall stack. */
        mov r12, rsp
        lea rsp, {0}

        /* Push fake stack frame. */
        push 0x0 /* Push empty stack seg value. */
        push rcx /* Push `rsp` value. */
        push r11 /* Push `rflags` value. */
        push 0x0 /* Push empty code seg value. */
        push r12 /* Push `rip` value. */

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
        push rdx
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
        lea rbx, {1}
        /* Calculate the absolute address of the desired handler. */
        mov rax, $0x8
        mul r10
        add rax, rbx    /* Absolute address of handler pointer. */
        mov rax, [rax]  /* Absolute address of handler. */

        call rax

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
        pop r12 /* Pop empty stack seg value. */
        pop r12 /* Pop cached `rsp` value. */
        pop r11 /* Pop cached `rflags` value. */
        pop rcx /* Pop empty code seg value. */
        pop rcx /* Pop cached `rip` value. */
        
        r: jmp r

        /* Restore previous stack. */
        mov rsp, r12
        
        sysret
        ",
        sym __syscall_stack,
        sym SYSCALL_FUNCTIONS,
        options(noreturn),
    );
}

#[no_mangle]
unsafe extern "win64" fn syscall_test(
    stack_frame: &mut InterruptStackFrame,
    cached_regs: *mut ThreadRegisters,
) {
    info!("{:#?}\n{:#?}", stack_frame, cached_regs.read_volatile());
}
