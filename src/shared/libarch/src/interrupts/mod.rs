use core::cell::SyncUnsafeCell;
use libcommon::{Address, Virtual};
use num_enum::TryFromPrimitive;

mod instructions;
pub use instructions::*;

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
    Timer = 0x30,
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

/// Indicates what type of error the common page fault handler encountered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFaultHandlerError {
    AddressNotMapped,
    NotDemandPaged,
    NoHandler,
}

type PageFaultHandler = fn(Address<Virtual>) -> Result<(), PageFaultHandlerError>;
pub(crate) static PAGE_FAULT_HANDLER: SyncUnsafeCell<PageFaultHandler> =
    SyncUnsafeCell::new(|_| Err(PageFaultHandlerError::NoHandler));
pub fn set_page_fault_handler(handler: PageFaultHandler) {
    // SAFETY: Changing a function pointer shouldn't result in UB with interrupts disabled.
    crate::interrupts::without(|| unsafe { PAGE_FAULT_HANDLER.get().write(handler) });
}

/* NON-EXCEPTION IRQ HANDLING */
#[cfg(target_arch = "x86_64")]
pub type ArchContext = (crate::x64::cpu::GeneralContext, crate::x64::cpu::SpecialContext);

type InterruptHandler = fn(u64, &mut ControlFlowContext, &mut ArchContext);
pub(crate) static INTERRUPT_HANDLER: SyncUnsafeCell<InterruptHandler> = SyncUnsafeCell::new(|_, _, _| {});
pub fn set_interrupt_handler(handler: InterruptHandler) {
    // SAFETY: Changing a function pointer shouldn't result in UB with interrupts disabled.
    crate::interrupts::without(|| unsafe { INTERRUPT_HANDLER.get().write(handler) });
}

#[cfg(target_arch = "x86_64")]
pub type SyscallContext = crate::x64::cpu::syscall::PreservedRegisters;
#[repr(C, packed)]
pub struct SyscallReturnContext {
    ip: u64,
    sp: u64,
}
pub type SyscallHandler = fn(
    vector: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    ret_ip: u64,
    ret_sp: u64,
    regs: &mut SyscallContext,
) -> SyscallReturnContext;
pub(crate) static SYSCALL_HANDLER: SyncUnsafeCell<SyscallHandler> =
    SyncUnsafeCell::new(|_, _, _, _, _, _, _, _, _| panic!("no system call handler"));
pub fn set_syscall_handler(syscall_handler: SyscallHandler) {
    // SAFETY: Changing a function pointer shouldn't result in UB when interrupts are disabled.
    crate::interrupts::without(|| unsafe { SYSCALL_HANDLER.get().write(syscall_handler) });
}
