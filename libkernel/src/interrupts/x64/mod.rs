//mod apic;
mod exceptions;
mod stubs;

pub use exceptions::*;
pub use stubs::*;
use x86_64::structures::idt::InterruptStackFrame;

use crate::{Address, Virtual};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(self) enum InterruptDeliveryMode {
    Fixed = 0b000,
    LowPriority = 0b001,
    SMI = 0b010,
    NMI = 0b100,
    INIT = 0b101,
    StartUp = 0b110,
    ExtINT = 0b111,
}

#[repr(C)]
pub struct GeneralRegisters {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

#[naked]
#[no_mangle]
pub(self) extern "x86-interrupt" fn irq_common(_: x86_64::structures::idt::InterruptStackFrame) {
    unsafe {
        core::arch::asm!(
        "
        # (QWORD) ISF should begin here on the stack. 
        # (QWORD) IRQ vector is here.
        # (QWORD) `call` return instruction pointer is here.

        # Push all gprs to the stack.
        push r15
        push r14
        push r13
        push r12
        push r11
        push r10
        push r9
        push r8
        push rbp
        push rdi
        push rsi
        push rdx
        push rcx
        push rbx
        push rax
    
        cld

        # Move IRQ vector into first parameter
        mov rdi, [rsp + (16 * 8)]
        # Move stack frame into second parameter.
        lea rsi, [rsp + (17 * 8)]
        # Move cached gprs pointer into third parameter.
        mov rdx, rsp

        call {}
    
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

        # 'pop' interrupt vector and return pointer
        add rsp, 0x10

        iretq
        ",
        sym irq_handoff,
        options(noreturn)
        );
    }
}

extern "sysv64" fn irq_handoff(irq_number: u64, stack_frame: &mut InterruptStackFrame, context: &mut GeneralRegisters) {
    super::get_common_interrupt_handler()(irq_number, stack_frame, context);
}
