mod instructions;
mod syscall;

pub use instructions::*;

use core::cell::SyncUnsafeCell;
use num_enum::TryFromPrimitive;

const PIC_BASE: u8 = 0xE0;

/// Delivery mode for IPIs.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptDeliveryMode {
    Fixed = 0b000,
    LowPriority = 0b001,
    SMI = 0b010,
    NMI = 0b100,
    INIT = 0b101,
    StartUp = 0b110,
    ExtINT = 0b111,
}

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
    Clock = 0x20,
    /* 0x21..=0x2F reserved for PIC */
    Syscall = 0x30,
    Timer = 0x31,
    Thermal = 0x32,
    Performance = 0x33,
    /* 0x34..=0x3B free for use */
    Error = 0x3C,
    LINT0 = 0x3D,
    LINT1 = 0x3E,
    SPURIOUS = 0x3F,
}

#[derive(Debug, Clone, Copy)]
pub struct ControlFlowContext {
    pub ip: u64,
    pub sp: u64,
}

#[cfg(target_arch = "x86_64")]
pub type ArchContext = (crate::arch::x64::cpu::GeneralContext, crate::arch::x64::cpu::SpecialContext);

#[cfg(target_arch = "x86_64")]
pub type ArchException = crate::arch::x64::structures::idt::Exception;

/* EXCEPTION HANDLING */
type ExceptionHandler = fn(ArchException);
static EXCEPTION_HANDLER: SyncUnsafeCell<ExceptionHandler> =
    SyncUnsafeCell::new(|exception| panic!("\n{:#?}", exception));

/// Gets the current common exception handler.
#[inline]
pub fn get_common_exception_handler() -> &'static ExceptionHandler {
    unsafe { &*EXCEPTION_HANDLER.get() }
}

/* NON-EXCEPTION IRQ HANDLING */
type InterruptHandler = fn(u64, &mut ControlFlowContext, &mut ArchContext);
static INTERRUPT_HANDLER: SyncUnsafeCell<InterruptHandler> = SyncUnsafeCell::new(common_interrupt_handler);

/// Gets the current common interrupt handler.
#[inline]
pub fn get_common_interrupt_handler() -> &'static InterruptHandler {
    unsafe { &*INTERRUPT_HANDLER.get() }
}

pub fn common_interrupt_handler(
    irq_vector: u64,
    ctrl_flow_context: &mut ControlFlowContext,
    arch_context: &mut ArchContext,
) {
    match Vector::try_from(irq_vector) {
        Ok(vector) if vector == Vector::Timer => {
            crate::local_state::schedule_next_task(ctrl_flow_context, arch_context);
        }

        Ok(vector) if vector == Vector::Syscall => {
            // TODO general syscall impl
            #[cfg(target_arch = "x86_64")]
            {
                arch_context.0.rax = syscall::syscall_handler(
                    arch_context.0.rdi,
                    arch_context.0.rsi,
                    arch_context.0.rdx,
                    arch_context.0.rcx,
                    arch_context.0.r8,
                    arch_context.0.r9,
                );
            }
        }

        vector_result => {
            warn!("Unhandled IRQ vector: {:?}", vector_result);
        }
    }

    #[cfg(target_arch = "x86_64")]
    crate::arch::x64::structures::apic::end_of_interrupt();
}
