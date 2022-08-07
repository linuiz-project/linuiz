//mod apic;

use crate::{Address, Virtual};
use bit_field::{BitArray, BitField};
use x86_64::structures::idt::PageFaultErrorCode;

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
pub struct Context {
    pub rip: Address<Virtual>,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: Address<Virtual>,
    pub ss: u64,
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

pub trait InterruptController {}

/// Type of table a [`SelectorErrorCode`] indexes.
#[derive(Debug)]
pub enum TableType {
    GDT,
    IDT,
    LDT,
}

#[repr(transparent)]
#[derive(Debug)]
pub struct SelectorErrorCode(u32);

impl SelectorErrorCode {
    /// Whether the exception this selector code originated from was generated
    /// externally from the processor.
    #[inline(always)]
    pub fn is_external(&self) -> bool {
        self.0.get_bit(0)
    }

    /// Returrns the table type this selector indexes.
    #[inline(always)]
    pub fn table_type(&self) -> TableType {
        match self.0.get_bits(1..3) {
            0b00 => TableType::GDT,
            0b01 | 0b11 => TableType::IDT,
            0b10 => TableType::LDT,
            _ => panic!("invalid table type"),
        }
    }

    /// Index of the table this selector points to.
    #[inline(always)]
    pub fn index(&self) -> u16 {
        self.0.get_bits(3..16) as u16
    }
}

bitflags::bitflags! {
    /// Error type for page faults.
    #[repr(transparent)]
    pub struct PageFaultError : u64 {
        /// When set, the fault was caused by a page protection violation. When not set, it was caused
        /// by a non-present page.
        const PROTECTION = 1 << 0;

        /// When set, the fault was caused by a write. When not set, it was caused by a read.
        const WRITE = 1 << 1;

        /// When set, the fault occured in usermode.
        ///
        /// REMARK: This does not necessarily mean the page fault was a protection violation.
        const USERMODE = 1 << 2;

        /// When set, the fault was caused by reserved bits in a page table entry or table being set.
        const RESERVED_BITS = 1 << 3;

        /// When set, the page fault was caused by an instruction fetch.
        const INSTRUCTION_FETCH = 1 << 4;

        /// When set, the page fault was caused by a protection-key violation.
        const PROTECTION_KEY = 1 << 5;

        /// When set, fault was caused by shadow stack access.
        const SHADOW_STACK = 1 << 6;

        /// When set, the fault was due to a Software Guard eXtensions violation.
        ///
        /// REMARK: Generally this fault is unrelated to ordinary paging.
        const SGX = 1 << 15;
    }
}

#[repr(C, u8)]
#[derive(Debug)]
pub enum Exception {
    /// Generated upon an attempt to divide by zero.
    DivideError,

    /// Exception generated due to various conditions, outlined within the IA-32 SDM.
    /// Debug registers will be updated to provide context to this exception.
    Debug,

    /// Typically caused by unrecoverable RAM or other hardware errors.
    NonMaskable,

    /// Occurs when `int3` is called in software.
    Breakpoint,

    /// Occurs when the `into` instruction is executed with the `OVERFLOW` bit set in RFlags.
    Overflow,

    /// Occurs when the `bound` instruction is executed and fails its check.
    BoundRangeExceeded,

    /// Occurs when the processor tries to execute an invalid or undefined opcode.
    InvalidOpcode,

    /// Generated when there is no FPU available, but an FPU-reliant instruction is executed.
    DeviceNotAvailable,

    /// Occurs when an exception is unhandled or when an exception occurs while the CPU is
    /// trying to call an exception handler.
    DoubleFault,

    /// Occurs when an invalid segment selector is referenced as part of a task switch, or as a
    /// result of a control transfer through a gate descriptor, which results in an invalid
    /// stack-segment reference using an SS selector in the TSS
    InvalidTSS(SelectorErrorCode),

    /// Occurs when trying to load a segment or gate which has its `PRESENT` bit unset.
    SegmentNotPresent(SelectorErrorCode),

    /// Occurs when:
    ///     - Loading a stack-segment referencing a segment descriptor which is not present;
    ///     - Any `push`/`pop` instruction or any instruction using `esp`/`ebp` as a base register
    ///         is executed, while the stack address is not in canonical form;
    ///     - The stack-limit check fails.
    StackSegmentFault(SelectorErrorCode),

    /// Occurs when:
    ///     - Segment error (privilege, type, limit, r/w rights).
    ///     - Executing a privileged instruction while CPL isn't supervisor (CPL0)
    ///     - Writing a `1` in a reserved register field or writing invalid value combinations (e.g. `CR0` with `PE` unset and `PG` set).
    ///     - Referencing or accessing a null descriptor.
    GeneralProtectionFault(SelectorErrorCode),

    /// Occurs when:
    ///     - A page directory or table entry is not present in physical memory.
    ///     - Attempting to load the instruction TLB with a translation for a non-executable page.
    ///     - A protection cehck (privilege, r/w) failed.
    ///     - A reserved bit in the page directory table or entries is set to 1.
    PageFault(PageFaultErrorCode, Address<Virtual>),

    /// Occurs when the `fwait` or `wait` instruction (or any floating point instruction) is executed, and the
    /// following conditions are true:
    ///     - `CR0.NE` is set.
    ///     - An unmasked x87 floating point exception is pending (i.e. the exception bit in the x87 floating point status-word register is set).
    x87FloatingPoint,

    /// Occurs when alignment checking is enabled and an unaligned memory data reference is performed.
    ///
    /// REMARK: Alignment checks are only performed when in usermode (CPL3).
    AlignmentCheck(u64),

    /// Exception is model-specific and processor implementations are not required to support it.
    ///
    /// REMARK: It uses model-specific registers (MSRs) to provide error information.
    ///         It is disabled by default. Set `CR4.MCE` to enable it.
    MachineCheck,

    /* VIRTUALIZATION EXCEPTIONS (not supported) */
    // /// Occurs when an unmasked 128-bit media floating-point exception occurs and the `CR4.OSXMMEXCPT` bit
    // /// is set. If it is not set, this error condition will trigger an invalid opcode exception instead.
    // SIMDFlaotingPoint,

    // /// Occurs only on processors that support setting the `EPT-violation` bit for VM execution control.
    // Virtualization,

    // /// Occurs under several conditions on the `ret`/`iret`/`rstorssp`/`setssbsy` instructions.
    // ControlProtection,

    // HypervisorInjection,

    // VMMCommunication,

    // Security,
    /// Not an exception; it will never be handled by an interrupt handler. It is included here for completeness.
    TripleFault,
}
