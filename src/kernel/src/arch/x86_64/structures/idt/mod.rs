#![allow(unused_unsafe)]

mod entry;
use entry::*;

mod stubs;
use stubs::*;

mod isf;
pub use isf::*;

mod error_codes;
pub use error_codes::*;

pub const DB_STACK_TABLE_INDEX: u16 = 0;
pub const NM_STACK_TABLE_INDEX: u16 = 1;
pub const DF_STACK_TABLE_INDEX: u16 = 2;
pub const MC_STACK_TABLE_INDEX: u16 = 3;

/// An Interrupt Descriptor Table with 256 entries.
///
/// The first 32 entries are used for CPU exceptions. These entries can be either accessed through
/// fields on this struct or through an index operation, e.g. `idt[0]` returns the
/// first entry, the entry for the `divide_error` exception. Note that the index access is
/// not possible for entries for which an error code is pushed.
///
/// The remaining entries are used for interrupts. They can be accessed through index
/// operations on the idt, e.g. `idt[32]` returns the first interrupt entry, which is the 32nd IDT
/// entry).
///
///
/// The field descriptions are taken from the
/// [AMD64 manual volume 2](https://support.amd.com/TechDocs/24593.pdf)
/// (with slight modifications).
#[derive(Debug, Clone)]
#[repr(C)]
#[repr(align(16))]
pub struct InterruptDescriptorTable {
    /// A divide error (`#DE`) occurs when the denominator of a DIV instruction or
    /// an IDIV instruction is 0. A `#DE` also occurs if the result is too large to be
    /// represented in the destination.
    ///
    /// The saved instruction pointer points to the instruction that caused the `#DE`.
    ///
    /// The vector number of the `#DE` exception is 0.
    pub divide_error: Entry<HandlerFunc>,

    /// When the debug-exception mechanism is enabled, a `#DB` exception can occur under any
    /// of the following circumstances:
    ///
    /// <details>
    ///
    /// - Instruction execution.
    /// - Instruction single stepping.
    /// - Data read.
    /// - Data write.
    /// - I/O read.
    /// - I/O write.
    /// - Task switch.
    /// - Debug-register access, or general detect fault (debug register access when DR7.GD=1).
    /// - Executing the INT1 instruction (opcode 0F1h).
    ///
    /// </details>
    ///
    /// `#DB` conditions are enabled and disabled using the debug-control register, `DR7`
    /// and `RFLAGS.TF`.
    ///
    /// In the following cases, the saved instruction pointer points to the instruction that
    /// caused the `#DB`:
    ///
    /// - Instruction execution.
    /// - Invalid debug-register access, or general detect.
    ///
    /// In all other cases, the instruction that caused the `#DB` is completed, and the saved
    /// instruction pointer points to the instruction after the one that caused the `#DB`.
    ///
    /// The vector number of the `#DB` exception is 1.
    pub debug: Entry<HandlerFunc>,

    /// An non maskable interrupt exception (NMI) occurs as a result of system logic
    /// signaling a non-maskable interrupt to the processor.
    ///
    /// The processor recognizes an NMI at an instruction boundary.
    /// The saved instruction pointer points to the instruction immediately following the
    /// boundary where the NMI was recognized.
    ///
    /// The vector number of the NMI exception is 2.
    pub non_maskable_interrupt: Entry<HandlerFunc>,

    /// A breakpoint (`#BP`) exception occurs when an `INT3` instruction is executed. The
    /// `INT3` is normally used by debug software to set instruction breakpoints by replacing
    ///
    /// The saved instruction pointer points to the byte after the `INT3` instruction.
    ///
    /// The vector number of the `#BP` exception is 3.
    pub breakpoint: Entry<HandlerFunc>,

    /// An overflow exception (`#OF`) occurs as a result of executing an `INTO` instruction
    /// while the overflow bit in `RFLAGS` is set to 1.
    ///
    /// The saved instruction pointer points to the instruction following the `INTO`
    /// instruction that caused the `#OF`.
    ///
    /// The vector number of the `#OF` exception is 4.
    pub overflow: Entry<HandlerFunc>,

    /// A bound-range exception (`#BR`) exception can occur as a result of executing
    /// the `BOUND` instruction. The `BOUND` instruction compares an array index (first
    /// operand) with the lower bounds and upper bounds of an array (second operand).
    /// If the array index is not within the array boundary, the `#BR` occurs.
    ///
    /// The saved instruction pointer points to the `BOUND` instruction that caused the `#BR`.
    ///
    /// The vector number of the `#BR` exception is 5.
    pub bound_range_exceeded: Entry<HandlerFunc>,

    /// An invalid opcode exception (`#UD`) occurs when an attempt is made to execute an
    /// invalid or undefined opcode. The validity of an opcode often depends on the
    /// processor operating mode.
    ///
    /// <details><summary>A `#UD` occurs under the following conditions:</summary>
    ///
    /// - Execution of any reserved or undefined opcode in any mode.
    /// - Execution of the `UD2` instruction.
    /// - Use of the `LOCK` prefix on an instruction that cannot be locked.
    /// - Use of the `LOCK` prefix on a lockable instruction with a non-memory target location.
    /// - Execution of an instruction with an invalid-operand type.
    /// - Execution of the `SYSENTER` or `SYSEXIT` instructions in long mode.
    /// - Execution of any of the following instructions in 64-bit mode: `AAA`, `AAD`,
    ///   `AAM`, `AAS`, `BOUND`, `CALL` (opcode 9A), `DAA`, `DAS`, `DEC`, `INC`, `INTO`,
    ///   `JMP` (opcode EA), `LDS`, `LES`, `POP` (`DS`, `ES`, `SS`), `POPA`, `PUSH` (`CS`,
    ///   `DS`, `ES`, `SS`), `PUSHA`, `SALC`.
    /// - Execution of the `ARPL`, `LAR`, `LLDT`, `LSL`, `LTR`, `SLDT`, `STR`, `VERR`, or
    ///   `VERW` instructions when protected mode is not enabled, or when virtual-8086 mode
    ///   is enabled.
    /// - Execution of any legacy SSE instruction when `CR4.OSFXSR` is cleared to 0.
    /// - Execution of any SSE instruction (uses `YMM`/`XMM` registers), or 64-bit media
    ///   instruction (uses `MMXTM` registers) when `CR0.EM` = 1.
    /// - Execution of any SSE floating-point instruction (uses `YMM`/`XMM` registers) that
    ///   causes a numeric exception when `CR4.OSXMMEXCPT` = 0.
    /// - Use of the `DR4` or `DR5` debug registers when `CR4.DE` = 1.
    /// - Execution of `RSM` when not in `SMM` mode.
    ///
    /// </details>
    ///
    /// The saved instruction pointer points to the instruction that caused the `#UD`.
    ///
    /// The vector number of the `#UD` exception is 6.
    pub invalid_opcode: Entry<HandlerFunc>,

    /// A device not available exception (`#NM`) occurs under any of the following conditions:
    ///
    /// <details>
    ///
    /// - An `FWAIT`/`WAIT` instruction is executed when `CR0.MP=1` and `CR0.TS=1`.
    /// - Any x87 instruction other than `FWAIT` is executed when `CR0.EM=1`.
    /// - Any x87 instruction is executed when `CR0.TS=1`. The `CR0.MP` bit controls whether the
    ///   `FWAIT`/`WAIT` instruction causes an `#NM` exception when `TS=1`.
    /// - Any 128-bit or 64-bit media instruction when `CR0.TS=1`.
    ///
    /// </details>
    ///
    /// The saved instruction pointer points to the instruction that caused the `#NM`.
    ///
    /// The vector number of the `#NM` exception is 7.
    pub device_not_available: Entry<HandlerFunc>,

    /// A double fault (`#DF`) exception can occur when a second exception occurs during
    /// the handling of a prior (first) exception or interrupt handler.
    ///
    /// <details>
    ///
    /// Usually, the first and second exceptions can be handled sequentially without
    /// resulting in a `#DF`. In this case, the first exception is considered _benign_, as
    /// it does not harm the ability of the processor to handle the second exception. In some
    /// cases, however, the first exception adversely affects the ability of the processor to
    /// handle the second exception. These exceptions contribute to the occurrence of a `#DF`,
    /// and are called _contributory exceptions_. The following exceptions are contributory:
    ///
    /// - Invalid-TSS Exception
    /// - Segment-Not-Present Exception
    /// - Stack Exception
    /// - General-Protection Exception
    ///
    /// A double-fault exception occurs in the following cases:
    ///
    /// - If a contributory exception is followed by another contributory exception.
    /// - If a divide-by-zero exception is followed by a contributory exception.
    /// - If a page  fault is followed by another page fault or a contributory exception.
    ///
    /// If a third interrupting event occurs while transferring control to the `#DF` handler,
    /// the processor shuts down.
    ///
    /// </details>
    ///
    /// The returned error code is always zero. The saved instruction pointer is undefined,
    /// and the program cannot be restarted.
    ///
    /// The vector number of the `#DF` exception is 8.
    pub double_fault: Entry<DivergingHandlerFuncWithErrCode>,

    /// This interrupt vector is reserved. It is for a discontinued exception originally used
    /// by processors that supported external x87-instruction coprocessors. On those processors,
    /// the exception condition is caused by an invalid-segment or invalid-page access on an
    /// x87-instruction coprocessor-instruction operand. On current processors, this condition
    /// causes a general-protection exception to occur.
    coprocessor_segment_overrun: Entry<HandlerFunc>,

    /// An invalid TSS exception (`#TS`) occurs only as a result of a control transfer through
    /// a gate descriptor that results in an invalid stack-segment reference using an `SS`
    /// selector in the TSS.
    ///
    /// The returned error code is the `SS` segment selector. The saved instruction pointer
    /// points to the control-transfer instruction that caused the `#TS`.
    ///
    /// The vector number of the `#TS` exception is 10.
    pub invalid_tss: Entry<HandlerFuncWithErrCode>,

    /// An segment-not-present exception (`#NP`) occurs when an attempt is made to load a
    /// segment or gate with a clear present bit.
    ///
    /// The returned error code is the segment-selector index of the segment descriptor
    /// causing the `#NP` exception. The saved instruction pointer points to the instruction
    /// that loaded the segment selector resulting in the `#NP`.
    ///
    /// The vector number of the `#NP` exception is 11.
    pub segment_not_present: Entry<HandlerFuncWithErrCode>,

    /// An stack segment exception (`#SS`) can occur in the following situations:
    ///
    /// - Implied stack references in which the stack address is not in canonical
    ///   form. Implied stack references include all push and pop instructions, and any
    ///   instruction using `RSP` or `RBP` as a base register.
    /// - Attempting to load a stack-segment selector that references a segment descriptor
    ///   containing a clear present bit.
    /// - Any stack access that fails the stack-limit check.
    ///
    /// The returned error code depends on the cause of the `#SS`. If the cause is a cleared
    /// present bit, the error code is the corresponding segment selector. Otherwise, the
    /// error code is zero. The saved instruction pointer points to the instruction that
    /// caused the `#SS`.
    ///
    /// The vector number of the `#NP` exception is 12.
    pub stack_segment_fault: Entry<HandlerFuncWithErrCode>,

    /// A general protection fault (`#GP`) can occur in various situations. Common causes include:
    ///
    /// - Executing a privileged instruction while `CPL > 0`.
    /// - Writing a 1 into any register field that is reserved, must be zero (MBZ).
    /// - Attempting to execute an SSE instruction specifying an unaligned memory operand.
    /// - Loading a non-canonical base address into the `GDTR` or `IDTR`.
    /// - Using WRMSR to write a read-only MSR.
    /// - Any long-mode consistency-check violation.
    ///
    /// The returned error code is a segment selector, if the cause of the `#GP` is
    /// segment-related, and zero otherwise. The saved instruction pointer points to
    /// the instruction that caused the `#GP`.
    ///
    /// The vector number of the `#GP` exception is 13.
    pub general_protection_fault: Entry<HandlerFuncWithErrCode>,

    /// A page fault (`#PF`) can occur during a memory access in any of the following situations:
    ///
    /// - A page-translation-table entry or physical page involved in translating the memory
    ///   access is not present in physical memory. This is indicated by a cleared present
    ///   bit in the translation-table entry.
    /// - An attempt is made by the processor to load the instruction TLB with a translation
    ///   for a non-executable page.
    /// - The memory access fails the paging-protection checks (user/supervisor, read/write,
    ///   or both).
    /// - A reserved bit in one of the page-translation-table entries is set to 1. A `#PF`
    ///   occurs for this reason only when `CR4.PSE=1` or `CR4.PAE=1`.
    ///
    /// The virtual (linear) address that caused the `#PF` is stored in the `CR2` register.
    /// The saved instruction pointer points to the instruction that caused the `#PF`.
    ///
    /// The page-fault error code is described by the
    /// [`PageFaultErrorCode`](struct.PageFaultErrorCode.html) struct.
    ///
    /// The vector number of the `#PF` exception is 14.
    pub page_fault: Entry<PageFaultHandlerFunc>,

    /// vector nr. 15
    _reserved1: [Entry<HandlerFunc>; 1],

    /// The x87 Floating-Point Exception-Pending exception (`#MF`) is used to handle unmasked x87
    /// floating-point exceptions. In 64-bit mode, the x87 floating point unit is not used
    /// anymore, so this exception is only relevant when executing programs in the 32-bit
    /// compatibility mode.
    ///
    /// The vector number of the `#MF` exception is 16.
    pub x87_floating_point: Entry<HandlerFunc>,

    /// An alignment check exception (`#AC`) occurs when an unaligned-memory data reference
    /// is performed while alignment checking is enabled. An `#AC` can occur only when CPL=3.
    ///
    /// The returned error code is always zero. The saved instruction pointer points to the
    /// instruction that caused the `#AC`.
    ///
    /// The vector number of the `#AC` exception is 17.
    pub alignment_check: Entry<HandlerFuncWithErrCode>,

    /// The machine check exception (`#MC`) is model specific. Processor implementations
    /// are not required to support the `#MC` exception, and those implementations that do
    /// support `#MC` can vary in how the `#MC` exception mechanism works.
    ///
    /// There is no reliable way to restart the program.
    ///
    /// The vector number of the `#MC` exception is 18.
    pub machine_check: Entry<DivergingHandlerFunc>,

    /// The SIMD Floating-Point Exception (`#XF`) is used to handle unmasked SSE
    /// floating-point exceptions. The SSE floating-point exceptions reported by
    /// the `#XF` exception are (including mnemonics):
    ///
    /// - IE: Invalid-operation exception (also called #I).
    /// - DE: Denormalized-operand exception (also called #D).
    /// - ZE: Zero-divide exception (also called #Z).
    /// - OE: Overflow exception (also called #O).
    /// - UE: Underflow exception (also called #U).
    /// - PE: Precision exception (also called #P or inexact-result exception).
    ///
    /// The saved instruction pointer points to the instruction that caused the `#XF`.
    ///
    /// The vector number of the `#XF` exception is 19.
    pub simd_floating_point: Entry<HandlerFunc>,

    /// vector nr. 20
    pub virtualization: Entry<HandlerFunc>,

    /// A #CP exception is generated when shadow stacks are enabled and mismatch
    /// scenarios are detected (possible error code cases below).
    ///
    /// The error code is the #CP error code, for each of the following situations:
    /// - A RET (near) instruction encountered a return address mismatch.
    /// - A RET (far) instruction encountered a return address mismatch.
    /// - A RSTORSSP instruction encountered an invalid shadow stack restore token.
    /// - A SETSSBY instruction encountered an invalid supervisor shadow stack token.
    /// - A missing ENDBRANCH instruction if indirect branch tracking is enabled.
    ///
    /// vector nr. 21
    pub cp_protection_exception: Entry<HandlerFuncWithErrCode>,

    /// vector nr. 22-27
    _reserved2: [Entry<HandlerFunc>; 6],

    /// The Hypervisor Injection Exception (`#HV`) is injected by a hypervisor
    /// as a doorbell to inform an `SEV-SNP` enabled guest running with the
    /// `Restricted Injection` feature of events to be processed.
    ///
    /// `SEV-SNP` stands for the _"Secure Nested Paging"_ feature of the _"AMD
    /// Secure Encrypted Virtualization"_  technology. The `Restricted
    /// Injection` feature disables all hypervisor-based interrupt queuing
    /// and event injection of all vectors except #HV.
    ///
    /// The `#HV` exception is a benign exception and can only be injected as
    /// an exception and without an error code. `SEV-SNP` enabled guests are
    /// expected to communicate with the hypervisor about events via a
    /// software-managed para-virtualization interface.
    ///
    /// The vector number of the ``#HV`` exception is 28.
    pub hv_injection_exception: Entry<HandlerFunc>,

    /// The VMM Communication Exception (`#VC`) is always generated by hardware when an `SEV-ES`
    /// enabled guest is running and an `NAE` event occurs.
    ///
    /// `SEV-ES` stands for the _"Encrypted State"_ feature of the _"AMD Secure Encrypted Virtualization"_
    /// technology. `NAE` stands for an _"Non-Automatic Exit"_, which is an `VMEXIT` event that requires
    /// hypervisor emulation. See
    /// [this whitepaper](https://www.amd.com/system/files/TechDocs/Protecting%20VM%20Register%20State%20with%20SEV-ES.pdf)
    /// for an overview of the `SEV-ES` feature.
    ///
    /// The `#VC` exception is a precise, contributory, fault-type exception utilizing exception vector 29.
    /// This exception cannot be masked. The error code of the `#VC` exception is equal
    /// to the `#VMEXIT` code of the event that caused the `NAE`.
    ///
    /// In response to a `#VC` exception, a typical flow would involve the guest handler inspecting the error
    /// code to determine the cause of the exception and deciding what register state must be copied to the
    /// `GHCB` (_"Guest Hypervisor Communication Block"_) for the event to be handled. The handler
    /// should then execute the `VMGEXIT` instruction to
    /// create an `AE` and invoke the hypervisor. After a later `VMRUN`, guest execution will resume after the
    /// `VMGEXIT` instruction where the handler can view the results from the hypervisor and copy state from
    /// the `GHCB` back to its internal state as needed.
    ///
    /// Note that it is inadvisable for the hypervisor to set the `VMCB` (_"Virtual Machine Control Block"_)
    /// intercept bit for the `#VC` exception as
    /// this would prevent proper handling of `NAE`s by the guest. Similarly, the hypervisor should avoid
    /// setting intercept bits for events that would occur in the `#VC` handler (such as `IRET`).
    ///
    /// The vector number of the ``#VC`` exception is 29.
    pub vmm_communication_exception: Entry<HandlerFuncWithErrCode>,

    /// The Security Exception (`#SX`) signals security-sensitive events that occur while
    /// executing the VMM, in the form of an exception so that the VMM may take appropriate
    /// action. (A VMM would typically intercept comparable sensitive events in the guest.)
    /// In the current implementation, the only use of the `#SX` is to redirect external INITs
    /// into an exception so that the VMM may — among other possibilities.
    ///
    /// The only error code currently defined is 1, and indicates redirection of INIT has occurred.
    ///
    /// The vector number of the ``#SX`` exception is 30.
    pub security_exception: Entry<HandlerFuncWithErrCode>,

    /// vector nr. 31
    _reserved3: [Entry<HandlerFunc>; 1],

    /// User-defined interrupts can be initiated either by system logic or software. They occur
    /// when:
    ///
    /// - System logic signals an external interrupt request to the processor. The signaling
    ///   mechanism and the method of communicating the interrupt vector to the processor are
    ///   implementation dependent.
    /// - Software executes an `INTn` instruction. The `INTn` instruction operand provides
    ///   the interrupt vector number.
    ///
    /// Both methods can be used to initiate an interrupt into vectors 0 through 255. However,
    /// because vectors 0 through 31 are defined or reserved by the AMD64 architecture,
    /// software should not use vectors in this range for purposes other than their defined use.
    ///
    /// The saved instruction pointer depends on the interrupt source:
    ///
    /// - External interrupts are recognized on instruction boundaries. The saved instruction
    ///   pointer points to the instruction immediately following the boundary where the
    ///   external interrupt was recognized.
    /// - If the interrupt occurs as a result of executing the INTn instruction, the saved
    ///   instruction pointer points to the instruction after the INTn.
    interrupts: [Entry<HandlerFunc>; 224],
}

impl core::ops::Index<u8> for InterruptDescriptorTable {
    type Output = Entry<HandlerFunc>;

    /// Returns the IDT entry with the specified index.
    ///
    /// Panics if the entry is an exception that pushes an error code (use the struct fields for accessing these entries).
    #[inline]
    fn index(&self, index: u8) -> &Self::Output {
        match index {
            index @ 32..=255 => &self.interrupts[usize::from(index) - 32],
            index => panic!("Exception vector '{index}' must be directly indexed."),
        }
    }
}

impl core::ops::IndexMut<u8> for InterruptDescriptorTable {
    /// Returns a mutable reference to the IDT entry with the specified index.
    ///
    /// Panics if the entry is an exception that pushes an error code (use the struct fields for accessing these entries).
    #[inline]
    fn index_mut(&mut self, index: u8) -> &mut Self::Output {
        match index {
            index @ 32..=255 => &mut self.interrupts[usize::from(index) - 32],
            index => panic!("Exception vector '{index}' must be directly indexed."),
        }
    }
}

static IDT: spin::Mutex<InterruptDescriptorTable> = spin::Mutex::new(InterruptDescriptorTable {
    divide_error: Entry::missing(),
    debug: Entry::missing(),
    non_maskable_interrupt: Entry::missing(),
    breakpoint: Entry::missing(),
    overflow: Entry::missing(),
    bound_range_exceeded: Entry::missing(),
    invalid_opcode: Entry::missing(),
    device_not_available: Entry::missing(),
    double_fault: Entry::missing(),
    coprocessor_segment_overrun: Entry::missing(),
    invalid_tss: Entry::missing(),
    segment_not_present: Entry::missing(),
    stack_segment_fault: Entry::missing(),
    general_protection_fault: Entry::missing(),
    page_fault: Entry::missing(),
    _reserved1: [Entry::missing(); _],
    x87_floating_point: Entry::missing(),
    alignment_check: Entry::missing(),
    machine_check: Entry::missing(),
    simd_floating_point: Entry::missing(),
    virtualization: Entry::missing(),
    cp_protection_exception: Entry::missing(),
    _reserved2: [Entry::missing(); _],
    hv_injection_exception: Entry::missing(),
    vmm_communication_exception: Entry::missing(),
    security_exception: Entry::missing(),
    _reserved3: [Entry::missing(); _],
    interrupts: [Entry::missing(); _],
});

#[allow(clippy::too_many_lines)]
pub fn load() {
    let mut idt = IDT.lock();

    // Safety: These are nominal and agreed-upon states for the given exception vectors.
    unsafe {
        idt.debug.set_handler_fn(db_handler).set_stack_index(DB_STACK_TABLE_INDEX);
        idt.non_maskable_interrupt.set_handler_fn(nmi_handler).set_stack_index(NM_STACK_TABLE_INDEX);
        idt.double_fault.set_handler_fn(df_handler).set_stack_index(DF_STACK_TABLE_INDEX);
        idt.machine_check.set_handler_fn(mc_handler).set_stack_index(MC_STACK_TABLE_INDEX);
    }

    idt.divide_error.set_handler_fn(de_handler);
    idt.breakpoint.set_handler_fn(bp_handler);
    idt.overflow.set_handler_fn(of_handler);
    idt.bound_range_exceeded.set_handler_fn(br_handler);
    idt.invalid_opcode.set_handler_fn(ud_handler);
    idt.device_not_available.set_handler_fn(na_handler);
    idt.invalid_tss.set_handler_fn(ts_handler);
    idt.segment_not_present.set_handler_fn(np_handler);
    idt.stack_segment_fault.set_handler_fn(ss_handler);
    idt.general_protection_fault.set_handler_fn(gp_handler);
    idt.page_fault.set_handler_fn(pf_handler);
    // --- 15    reserved
    idt.x87_floating_point.set_handler_fn(mf_handler);
    idt.alignment_check.set_handler_fn(ac_handler);
    idt.simd_floating_point.set_handler_fn(xm_handler);
    idt.virtualization.set_handler_fn(ve_handler);
    // --- 20-31 reserved
    // --- 32    triple fault (can't handle)

    // userspace syscall vector
    idt[128].set_handler_fn(irq_128).set_privilege_level(super::gdt::PrivilegeLevel::Ring3);

    idt[32].set_handler_fn(irq_32);
    idt[33].set_handler_fn(irq_33);
    idt[34].set_handler_fn(irq_34);
    idt[35].set_handler_fn(irq_35);
    idt[36].set_handler_fn(irq_36);
    idt[37].set_handler_fn(irq_37);
    idt[38].set_handler_fn(irq_38);
    idt[39].set_handler_fn(irq_39);
    idt[40].set_handler_fn(irq_40);
    idt[41].set_handler_fn(irq_41);
    idt[42].set_handler_fn(irq_42);
    idt[43].set_handler_fn(irq_43);
    idt[44].set_handler_fn(irq_44);
    idt[45].set_handler_fn(irq_45);
    idt[46].set_handler_fn(irq_46);
    idt[47].set_handler_fn(irq_47);
    idt[48].set_handler_fn(irq_48);
    idt[49].set_handler_fn(irq_49);
    idt[50].set_handler_fn(irq_50);
    idt[51].set_handler_fn(irq_51);
    idt[52].set_handler_fn(irq_52);
    idt[53].set_handler_fn(irq_53);
    idt[54].set_handler_fn(irq_54);
    idt[55].set_handler_fn(irq_55);
    idt[56].set_handler_fn(irq_56);
    idt[57].set_handler_fn(irq_57);
    idt[58].set_handler_fn(irq_58);
    idt[59].set_handler_fn(irq_59);
    idt[60].set_handler_fn(irq_60);
    idt[61].set_handler_fn(irq_61);
    idt[62].set_handler_fn(irq_62);
    idt[63].set_handler_fn(irq_63);
    idt[64].set_handler_fn(irq_64);
    idt[65].set_handler_fn(irq_65);
    idt[66].set_handler_fn(irq_66);
    idt[67].set_handler_fn(irq_67);
    idt[68].set_handler_fn(irq_68);
    idt[69].set_handler_fn(irq_69);
    idt[70].set_handler_fn(irq_70);
    idt[71].set_handler_fn(irq_71);
    idt[72].set_handler_fn(irq_72);
    idt[73].set_handler_fn(irq_73);
    idt[74].set_handler_fn(irq_74);
    idt[75].set_handler_fn(irq_75);
    idt[76].set_handler_fn(irq_76);
    idt[77].set_handler_fn(irq_77);
    idt[78].set_handler_fn(irq_78);
    idt[79].set_handler_fn(irq_79);
    idt[80].set_handler_fn(irq_80);
    idt[81].set_handler_fn(irq_81);
    idt[82].set_handler_fn(irq_82);
    idt[83].set_handler_fn(irq_83);
    idt[84].set_handler_fn(irq_84);
    idt[85].set_handler_fn(irq_85);
    idt[86].set_handler_fn(irq_86);
    idt[87].set_handler_fn(irq_87);
    idt[88].set_handler_fn(irq_88);
    idt[89].set_handler_fn(irq_89);
    idt[90].set_handler_fn(irq_90);
    idt[91].set_handler_fn(irq_91);
    idt[92].set_handler_fn(irq_92);
    idt[93].set_handler_fn(irq_93);
    idt[94].set_handler_fn(irq_94);
    idt[95].set_handler_fn(irq_95);
    idt[96].set_handler_fn(irq_96);
    idt[97].set_handler_fn(irq_97);
    idt[98].set_handler_fn(irq_98);
    idt[99].set_handler_fn(irq_99);
    idt[100].set_handler_fn(irq_100);
    idt[101].set_handler_fn(irq_101);
    idt[102].set_handler_fn(irq_102);
    idt[103].set_handler_fn(irq_103);
    idt[104].set_handler_fn(irq_104);
    idt[105].set_handler_fn(irq_105);
    idt[106].set_handler_fn(irq_106);
    idt[107].set_handler_fn(irq_107);
    idt[108].set_handler_fn(irq_108);
    idt[109].set_handler_fn(irq_109);
    idt[110].set_handler_fn(irq_110);
    idt[111].set_handler_fn(irq_111);
    idt[112].set_handler_fn(irq_112);
    idt[113].set_handler_fn(irq_113);
    idt[114].set_handler_fn(irq_114);
    idt[115].set_handler_fn(irq_115);
    idt[116].set_handler_fn(irq_116);
    idt[117].set_handler_fn(irq_117);
    idt[118].set_handler_fn(irq_118);
    idt[119].set_handler_fn(irq_119);
    idt[120].set_handler_fn(irq_120);
    idt[121].set_handler_fn(irq_121);
    idt[122].set_handler_fn(irq_122);
    idt[123].set_handler_fn(irq_123);
    idt[124].set_handler_fn(irq_124);
    idt[125].set_handler_fn(irq_125);
    idt[126].set_handler_fn(irq_126);
    idt[127].set_handler_fn(irq_127);
    idt[129].set_handler_fn(irq_129);
    idt[130].set_handler_fn(irq_130);
    idt[131].set_handler_fn(irq_131);
    idt[132].set_handler_fn(irq_132);
    idt[133].set_handler_fn(irq_133);
    idt[134].set_handler_fn(irq_134);
    idt[135].set_handler_fn(irq_135);
    idt[136].set_handler_fn(irq_136);
    idt[137].set_handler_fn(irq_137);
    idt[138].set_handler_fn(irq_138);
    idt[139].set_handler_fn(irq_139);
    idt[140].set_handler_fn(irq_140);
    idt[141].set_handler_fn(irq_141);
    idt[142].set_handler_fn(irq_142);
    idt[143].set_handler_fn(irq_143);
    idt[144].set_handler_fn(irq_144);
    idt[145].set_handler_fn(irq_145);
    idt[146].set_handler_fn(irq_146);
    idt[147].set_handler_fn(irq_147);
    idt[148].set_handler_fn(irq_148);
    idt[149].set_handler_fn(irq_149);
    idt[150].set_handler_fn(irq_150);
    idt[151].set_handler_fn(irq_151);
    idt[152].set_handler_fn(irq_152);
    idt[153].set_handler_fn(irq_153);
    idt[154].set_handler_fn(irq_154);
    idt[155].set_handler_fn(irq_155);
    idt[156].set_handler_fn(irq_156);
    idt[157].set_handler_fn(irq_157);
    idt[158].set_handler_fn(irq_158);
    idt[159].set_handler_fn(irq_159);
    idt[160].set_handler_fn(irq_160);
    idt[161].set_handler_fn(irq_161);
    idt[162].set_handler_fn(irq_162);
    idt[163].set_handler_fn(irq_163);
    idt[164].set_handler_fn(irq_164);
    idt[165].set_handler_fn(irq_165);
    idt[166].set_handler_fn(irq_166);
    idt[167].set_handler_fn(irq_167);
    idt[168].set_handler_fn(irq_168);
    idt[169].set_handler_fn(irq_169);
    idt[170].set_handler_fn(irq_170);
    idt[171].set_handler_fn(irq_171);
    idt[172].set_handler_fn(irq_172);
    idt[173].set_handler_fn(irq_173);
    idt[174].set_handler_fn(irq_174);
    idt[175].set_handler_fn(irq_175);
    idt[176].set_handler_fn(irq_176);
    idt[177].set_handler_fn(irq_177);
    idt[178].set_handler_fn(irq_178);
    idt[179].set_handler_fn(irq_179);
    idt[180].set_handler_fn(irq_180);
    idt[181].set_handler_fn(irq_181);
    idt[182].set_handler_fn(irq_182);
    idt[183].set_handler_fn(irq_183);
    idt[184].set_handler_fn(irq_184);
    idt[185].set_handler_fn(irq_185);
    idt[186].set_handler_fn(irq_186);
    idt[187].set_handler_fn(irq_187);
    idt[188].set_handler_fn(irq_188);
    idt[189].set_handler_fn(irq_189);
    idt[190].set_handler_fn(irq_190);
    idt[191].set_handler_fn(irq_191);
    idt[192].set_handler_fn(irq_192);
    idt[193].set_handler_fn(irq_193);
    idt[194].set_handler_fn(irq_194);
    idt[195].set_handler_fn(irq_195);
    idt[196].set_handler_fn(irq_196);
    idt[197].set_handler_fn(irq_197);
    idt[198].set_handler_fn(irq_198);
    idt[199].set_handler_fn(irq_199);
    idt[200].set_handler_fn(irq_200);
    idt[201].set_handler_fn(irq_201);
    idt[202].set_handler_fn(irq_202);
    idt[203].set_handler_fn(irq_203);
    idt[204].set_handler_fn(irq_204);
    idt[205].set_handler_fn(irq_205);
    idt[206].set_handler_fn(irq_206);
    idt[207].set_handler_fn(irq_207);
    idt[208].set_handler_fn(irq_208);
    idt[209].set_handler_fn(irq_209);
    idt[210].set_handler_fn(irq_210);
    idt[211].set_handler_fn(irq_211);
    idt[212].set_handler_fn(irq_212);
    idt[213].set_handler_fn(irq_213);
    idt[214].set_handler_fn(irq_214);
    idt[215].set_handler_fn(irq_215);
    idt[216].set_handler_fn(irq_216);
    idt[217].set_handler_fn(irq_217);
    idt[218].set_handler_fn(irq_218);
    idt[219].set_handler_fn(irq_219);
    idt[220].set_handler_fn(irq_220);
    idt[221].set_handler_fn(irq_221);
    idt[222].set_handler_fn(irq_222);
    idt[223].set_handler_fn(irq_223);
    idt[224].set_handler_fn(irq_224);
    idt[225].set_handler_fn(irq_225);
    idt[226].set_handler_fn(irq_226);
    idt[227].set_handler_fn(irq_227);
    idt[228].set_handler_fn(irq_228);
    idt[229].set_handler_fn(irq_229);
    idt[230].set_handler_fn(irq_230);
    idt[231].set_handler_fn(irq_231);
    idt[232].set_handler_fn(irq_232);
    idt[233].set_handler_fn(irq_233);
    idt[234].set_handler_fn(irq_234);
    idt[235].set_handler_fn(irq_235);
    idt[236].set_handler_fn(irq_236);
    idt[237].set_handler_fn(irq_237);
    idt[238].set_handler_fn(irq_238);
    idt[239].set_handler_fn(irq_239);
    idt[240].set_handler_fn(irq_240);
    idt[241].set_handler_fn(irq_241);
    idt[242].set_handler_fn(irq_242);
    idt[243].set_handler_fn(irq_243);
    idt[244].set_handler_fn(irq_244);
    idt[245].set_handler_fn(irq_245);
    idt[246].set_handler_fn(irq_246);
    idt[247].set_handler_fn(irq_247);
    idt[248].set_handler_fn(irq_248);
    idt[249].set_handler_fn(irq_249);
    idt[250].set_handler_fn(irq_250);
    idt[251].set_handler_fn(irq_251);
    idt[252].set_handler_fn(irq_252);
    idt[253].set_handler_fn(irq_253);
    idt[254].set_handler_fn(irq_254);
    idt[255].set_handler_fn(irq_255);

    let idt_dtptr = crate::arch::x86_64::structures::DescriptorTablePointer {
        limit: u16::try_from(core::mem::size_of_val(&*idt) - 1).unwrap(),
        base: u64::try_from((&raw const *idt).addr()).unwrap(),
    };

    // Safety: The descriptor table pointer was properly constructed.
    unsafe {
        core::arch::asm!(
            "lidt [{}]",
            in(reg) &idt_dtptr,
            options(readonly, nostack, preserves_flags)
        );
    }
}

// #[allow(clippy::too_many_lines)]
// pub fn set_stub_handlers(idt: &mut InterruptDescriptorTable) {
//     // userspace syscall vector
//     idt[128].set_handler_fn(irq_128).set_privilege_level(ia32utils::PrivilegeLevel::Ring3);
// }
