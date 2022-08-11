mod exceptions;
mod stubs;

pub mod pic8259;
pub use exceptions::*;
pub use stubs::*;

use core::cell::SyncUnsafeCell;
use libkernel::cpu::GeneralRegisters;
use num_enum::TryFromPrimitive;
use x86_64::structures::idt::InterruptStackFrame;

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[allow(non_camel_case_types)]
pub enum Vector {
    Syscall = 0x80,
    Timer = 0xA0,
    /* 224-256 ARCH SPECIFIC */
}

pub fn common_interrupt_handler(
    irq_vector: u64,
    stack_frame: &mut x86_64::structures::idt::InterruptStackFrame,
    context: &mut GeneralRegisters,
) {
    match Vector::try_from(irq_vector) {
        Ok(vector) => match vector {
            Vector::Syscall => todo!(),
            Vector::Timer => todo!(),
            Vector::Performance => todo!(),
            Vector::ThermalSensor => todo!(),
            Vector::Error => todo!(),
            Vector::LINT0_VECTOR | Vector::LINT1_VECTOR | Vector::SPURIOUS_VECTOR => {}
        },
        Err(vector_raw) => warn!("Unhandled IRQ vector: {:?}", vector_raw),
    }

    // TODO abstract this
    libkernel::structures::apic::end_of_interrupt();
}

/* EXCEPTION HANDLING */
type ExceptionHandler = fn(Exception);
static EXCEPTION_HANDLER: SyncUnsafeCell<ExceptionHandler> =
    SyncUnsafeCell::new(|exception| panic!("\n{:#?}", exception));

/// Sets the common exception handler.
///
/// SAFETY: The caller must ensure the provided function handles exceptions in a valid way.
pub unsafe fn set_common_exception_handler(handler: ExceptionHandler) {
    *EXCEPTION_HANDLER.get() = handler;
}

/// Gets the current common exception handler.
#[inline]
pub(self) fn get_common_exception_handler() -> &'static ExceptionHandler {
    unsafe { &*EXCEPTION_HANDLER.get() }
}

/* NON-EXCEPTION IRQ HANDLING */
type InterruptHandler = fn(u64, &mut InterruptStackFrame, &mut GeneralRegisters);
static INTERRUPT_HANDLER: SyncUnsafeCell<InterruptHandler> =
    SyncUnsafeCell::new(|irq_num, _, _| panic!("IRQ{}: no common handler", irq_num));

/// Sets the common interrupt handler.
///
/// SAFETY: The caller must ensure the provided function handles interrupts in a valid way.
pub unsafe fn set_common_interrupt_handler(handler: InterruptHandler) {
    *INTERRUPT_HANDLER.get() = handler;
}

/// Gets the current common interrupt handler.
#[inline]
pub(self) fn get_common_interrupt_handler() -> &'static InterruptHandler {
    unsafe { &*INTERRUPT_HANDLER.get() }
}

const PIC_BASE: u8 = 0xE0;
const PERFORMANCE: u8 = 0xF0;
const THERMAL_SENSOR: u8 = 0xF1;
const ERROR: u8 = 0xFC;
const LINT0_VECTOR: u8 = 0xFD;
const LINT1_VECTOR: u8 = 0xFE;
const SPURIOUS_VECTOR: u8 = 0xFF;

/// Delivery mode for IPIs.
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

/// Save all relevant data for eventual interrupt handler context.
#[naked]
#[no_mangle]
pub(self) extern "x86-interrupt" fn irq_save_context(_: x86_64::structures::idt::InterruptStackFrame) {
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

/// Hand the interrupt context off to the common interrupt handler.
extern "sysv64" fn irq_handoff(
    irq_number: u64,
    stack_frame: &mut InterruptStackFrame,
    context: &mut libkernel::cpu::GeneralRegisters,
) {
    super::get_common_interrupt_handler()(irq_number, stack_frame, context);
}
