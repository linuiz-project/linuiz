mod x64;

#[cfg(target_arch = "x86_64")]
pub use x64::*;

use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};

use crate::{Address, Virtual};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Vector {
    GlobalTimer = 0x20,
    Error = 0x40,
    LocalTimer = 0x41,
    Performance = 0x46,
    ThermalSensor = 0x47,
    Storage0 = 0x50,
    Storage1 = 0x51,
    Storage2 = 0x52,
    Syscall = 0x80,
    /* CANNOT BE CHANGED — DEFAULT FROM APIC */
    LINT0_VECTOR = 253,
    LINT1_VECTOR = 254,
    SPURIOUS_VECTOR = 255,
}

#[cfg(target_arch = "x86_64")]
pub type HandlerFunc = fn(&mut InterruptStackFrame, &mut crate::cpu::ThreadRegisters);

static INTERRUPT_HANDLERS: spin::RwLock<[Option<HandlerFunc>; 256]> = spin::RwLock::new([None; 256]);

/// Sets the interrupt handler function for the given vector.
///
/// SAFETY: This function is unsafe because any (including a malformed or buggy) handler can be
///         specified. The caller of this function must ensure the handler is correctly formed,
///         and properly handles the interrupt it is being assigned to.
pub unsafe fn set_handler_fn(vector: Vector, handler: HandlerFunc) {
    crate::instructions::interrupts::without_interrupts(|| {
        INTERRUPT_HANDLERS.write()[vector as usize] = Some(handler);
    });
}

// #[naked]
// pub extern "x86-interrupt" fn page_fault_handler(stack_frame: InterruptStackFrame, error_code: PageFaultErrorCode) {
//     unsafe {
//         core::arch::asm!(
//             "
//             sub rsp, 0x10                       # Make room for exception enum

//             # Move error code to union position
//             push rax
//             mov rax, [rsp + 0x18]
//             mov [rsp + 0x10], rax
//             pop rax

//             mov qword ptr [rsp + 0x10], 0xE      # Move exception discriminant

//             # Move excepting address to union
//             push rax
//             mov rax, cr2            # Move excepting address to register
//             mov [rsp + 0x8], rax    # Move the excepting address to stack
//             pop rax                 # Restore `rax`

//             call {}
//             ",
//             sym __page_fault_handler,
//             options(noreturn)
//         )
//     }
// }

pub extern "x86-interrupt" fn page_fault_handler(stack_frame: InterruptStackFrame, error_code: PageFaultErrorCode) {
    __page_fault_handler(stack_frame, Exception::PageFault(error_code, crate::registers::x64::control::CR2::read()))
}

#[no_mangle]
extern "C" fn __page_fault_handler(stack_frame: InterruptStackFrame, exception: Exception) {
    info!("{:#X}", core::mem::size_of::<Exception>());

    panic!("\n{:#?}", exception);
}

type ExceptionHandler = extern "C" fn(&mut InterruptStackFrame, &mut Context, &Exception);
type InterruptHandler = fn(InterruptStackFrame, &mut Context);

pub trait InterruptController {
    fn new() -> Self;
    fn set_exception_handler(handler: ExceptionHandler);
    fn set_interrupt_handler(handler: InterruptHandler);
}

// // TODO handle arch-specific exceptions
// #[]
// #[repr(C, u8)]
// pub enum Vector {
//     Syscall = 0x80,
//     Timer = 0xA0,
//     Performance = 0xA1,
//     ThermalSensor = 0xA2,

//     /* CANNOT BE CHANGED — DEFAULT FROM APIC */
//     Error = 0xFC,
//     LINT0_VECTOR = 0xFD,
//     LINT1_VECTOR = 0xFE,
//     SPURIOUS_VECTOR = 0xFF,
// }
