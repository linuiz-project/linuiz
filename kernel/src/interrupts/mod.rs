mod exceptions;
mod stubs;

pub mod apic;
pub mod ioapic;
pub mod pic;
pub use exceptions::*;
pub use stubs::*;

use core::cell::SyncUnsafeCell;
use libkernel::cpu::GeneralRegisters;
use num_enum::TryFromPrimitive;
use x86_64::structures::idt::InterruptStackFrame;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum DeliveryMode {
    Fixed = 0b000,
    LowPriority = 0b001,
    SMI = 0b010,
    NMI = 0b100,
    INIT = 0b101,
    StartUp = 0b110,
    ExtINT = 0b111,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationMode {
    Physical = 0,
    Logical = 1,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[allow(non_camel_case_types)]
pub enum Vector {
    Clock = 0xE0,

    Syscall = 0xF0,
    Timer = 0xF1,
    Thermal = 0xF2,
    Performance = 0xF3,
    /* 0xF4..0xFC free for use */
    Error = 0xFC,
    LINT0 = 0xFD,
    LINT1 = 0xFE,
    SPURIOUS = 0xFF,
}

pub fn common_interrupt_handler(
    irq_vector: u64,
    stack_frame: &mut x86_64::structures::idt::InterruptStackFrame,
    cached_regs: &mut GeneralRegisters,
) {
    match Vector::try_from(irq_vector) {
        // Allow external and spurious interrupts to do nothing
        Ok(vector) if vector == Vector::Timer => {
            crate::local_state::schedule_next_task(stack_frame, cached_regs);
            crate::interrupts::apic::end_of_interrupt();
        }

        Ok(vector) if vector == Vector::Syscall => {
            let control_ptr = cached_regs.rdi as *mut libkernel::syscall::Control;

            if !crate::memory::get_kernel_page_manager()
                .is_mapped(libkernel::Address::<libkernel::Virtual>::from_ptr(control_ptr))
            {
                cached_regs.rsi = libkernel::syscall::Error::ControlNotMapped as u64;
                return;
            }

            cached_regs.rsi = 0xDEADC0DE;
        }

        Ok(vector) if matches!(vector, Vector::LINT0 | Vector::LINT1 | Vector::SPURIOUS) => {}
        vector_result => {
            warn!("Unhandled IRQ vector: {:?}", vector_result);
            crate::interrupts::apic::end_of_interrupt();
        }
    }
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
static INTERRUPT_HANDLER: SyncUnsafeCell<InterruptHandler> = SyncUnsafeCell::new(common_interrupt_handler);

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
    get_common_interrupt_handler()(irq_number, stack_frame, context);
}
