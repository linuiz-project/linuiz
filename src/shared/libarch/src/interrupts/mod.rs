mod instructions;

pub use instructions::*;
use libcommon::{Address, LinkerSymbol, Virtual};
use num_enum::TryFromPrimitive;
use spin::Lazy;

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

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ControlFlowContext {
    pub ip: u64,
    pub sp: u64,
}

#[cfg(target_arch = "x86_64")]
pub type ArchContext = (crate::x64::cpu::GeneralContext, crate::x64::cpu::SpecialContext);

#[cfg(target_arch = "x86_64")]
pub type SyscallContext = crate::x64::cpu::syscall::PreservedRegisters;

/// Indicates what type of error the common page fault handler encountered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFaultHandlerError {
    AddressNotMapped,
    NotDemandPaged,
}

type PageFaultHandler = unsafe fn(Address<Virtual>) -> Result<(), PageFaultHandlerError>;
pub(crate) static PAGE_FAULT_HANDLER: Lazy<PageFaultHandler> = Lazy::new(|| unsafe {
    extern "C" {
        static __pf_handler: LinkerSymbol;
    }

    core::mem::transmute(__pf_handler.as_usize())
});

type IrqHandler = unsafe fn(u64, &mut ControlFlowContext, &mut ArchContext);
pub(crate) static IRQ_HANDLER: Lazy<IrqHandler> = Lazy::new(|| unsafe {
    extern "C" {
        static __irq_handler: LinkerSymbol;
    }

    core::mem::transmute(__irq_handler.as_usize())
});

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
) -> ControlFlowContext;
pub(crate) static SYSCALL_HANDLER: Lazy<SyscallHandler> = Lazy::new(|| unsafe {
    extern "C" {
        static __syscall_handler: LinkerSymbol;
    }

    core::mem::transmute(__syscall_handler.as_usize())
});
