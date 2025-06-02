#![allow(unused_unsafe)]

mod entry;
use entry::*;

mod stubs;
use stubs::*;

mod isf;
pub use isf::*;

mod error_codes;
pub use error_codes::*;

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackTableIndex {
    Debug = 0,
    NonMaskableInterrupt = 1,
    DoubleFault = 2,
    MachineCheck = 3,
}

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
#[repr(C)]
#[derive(Debug, Clone)]
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
    /// - If the interrupt occurs as a result of executing the `INTn` instruction, the saved
    ///   instruction pointer points to the instruction after the `INTn`.
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

static IDT: spin::Once<InterruptDescriptorTable> = spin::Once::new();

#[allow(clippy::too_many_lines)]
pub fn load() {
    IDT.call_once(|| InterruptDescriptorTable {
        divide_error: Entry::new(de_stub),
        // Safety: Stack table index is set to `Debug` stack.
        debug: unsafe { Entry::new_with_stack(db_stub, StackTableIndex::Debug) },
        // Safety: Stack table index is set to `NonMaskableInterrupt` stack.
        non_maskable_interrupt: unsafe { Entry::new_with_stack(nmi_stub, StackTableIndex::NonMaskableInterrupt) },
        breakpoint: Entry::new(bp_stub),
        overflow: Entry::new(of_stub),
        bound_range_exceeded: Entry::new(br_stub),
        invalid_opcode: Entry::new(ud_stub),
        device_not_available: Entry::new(na_stub),
        // Safety: Stack table index is set to `DoubleFault` stack.
        double_fault: unsafe { Entry::new_with_stack(df_stub, StackTableIndex::DoubleFault) },
        coprocessor_segment_overrun: Entry::missing(),
        invalid_tss: Entry::new(ts_stub),
        segment_not_present: Entry::new(np_stub),
        stack_segment_fault: Entry::new(ss_stub),
        general_protection_fault: Entry::new(gp_stub),
        page_fault: Entry::new(pf_stub),
        _reserved1: [Entry::missing(); _],
        x87_floating_point: Entry::new(mf_stub),
        alignment_check: Entry::new(ac_stub),
        // Safety: Stack table index is set to `MachineCheck` stack.
        machine_check: unsafe { Entry::new_with_stack(mc_stub, StackTableIndex::MachineCheck) },
        simd_floating_point: Entry::new(xm_stub),
        virtualization: Entry::new(ve_stub),
        cp_protection_exception: Entry::missing(),
        _reserved2: [Entry::missing(); _],
        hv_injection_exception: Entry::missing(),
        vmm_communication_exception: Entry::missing(),
        security_exception: Entry::missing(),
        _reserved3: [Entry::missing(); _],
        interrupts: [
            // Safety: Privilege level is set for coming FROM userspace (ring 3) for syscalls.
            unsafe { Entry::new_with_privilege(irq_128, super::gdt::PrivilegeLevel::Ring3) },
            Entry::new(irq_32),
            Entry::new(irq_33),
            Entry::new(irq_34),
            Entry::new(irq_35),
            Entry::new(irq_36),
            Entry::new(irq_37),
            Entry::new(irq_39),
            Entry::new(irq_38),
            Entry::new(irq_40),
            Entry::new(irq_41),
            Entry::new(irq_42),
            Entry::new(irq_43),
            Entry::new(irq_44),
            Entry::new(irq_45),
            Entry::new(irq_46),
            Entry::new(irq_47),
            Entry::new(irq_48),
            Entry::new(irq_49),
            Entry::new(irq_50),
            Entry::new(irq_51),
            Entry::new(irq_52),
            Entry::new(irq_53),
            Entry::new(irq_54),
            Entry::new(irq_55),
            Entry::new(irq_56),
            Entry::new(irq_57),
            Entry::new(irq_58),
            Entry::new(irq_59),
            Entry::new(irq_60),
            Entry::new(irq_61),
            Entry::new(irq_62),
            Entry::new(irq_63),
            Entry::new(irq_64),
            Entry::new(irq_65),
            Entry::new(irq_66),
            Entry::new(irq_67),
            Entry::new(irq_68),
            Entry::new(irq_69),
            Entry::new(irq_70),
            Entry::new(irq_71),
            Entry::new(irq_72),
            Entry::new(irq_73),
            Entry::new(irq_74),
            Entry::new(irq_75),
            Entry::new(irq_76),
            Entry::new(irq_77),
            Entry::new(irq_78),
            Entry::new(irq_79),
            Entry::new(irq_80),
            Entry::new(irq_81),
            Entry::new(irq_82),
            Entry::new(irq_83),
            Entry::new(irq_84),
            Entry::new(irq_85),
            Entry::new(irq_86),
            Entry::new(irq_87),
            Entry::new(irq_88),
            Entry::new(irq_89),
            Entry::new(irq_90),
            Entry::new(irq_91),
            Entry::new(irq_92),
            Entry::new(irq_93),
            Entry::new(irq_94),
            Entry::new(irq_95),
            Entry::new(irq_96),
            Entry::new(irq_97),
            Entry::new(irq_98),
            Entry::new(irq_99),
            Entry::new(irq_100),
            Entry::new(irq_101),
            Entry::new(irq_102),
            Entry::new(irq_103),
            Entry::new(irq_104),
            Entry::new(irq_105),
            Entry::new(irq_106),
            Entry::new(irq_107),
            Entry::new(irq_108),
            Entry::new(irq_109),
            Entry::new(irq_110),
            Entry::new(irq_111),
            Entry::new(irq_112),
            Entry::new(irq_113),
            Entry::new(irq_114),
            Entry::new(irq_115),
            Entry::new(irq_116),
            Entry::new(irq_117),
            Entry::new(irq_118),
            Entry::new(irq_119),
            Entry::new(irq_120),
            Entry::new(irq_121),
            Entry::new(irq_122),
            Entry::new(irq_123),
            Entry::new(irq_124),
            Entry::new(irq_125),
            Entry::new(irq_126),
            Entry::new(irq_127),
            Entry::new(irq_129),
            Entry::new(irq_130),
            Entry::new(irq_131),
            Entry::new(irq_132),
            Entry::new(irq_133),
            Entry::new(irq_134),
            Entry::new(irq_135),
            Entry::new(irq_136),
            Entry::new(irq_137),
            Entry::new(irq_138),
            Entry::new(irq_139),
            Entry::new(irq_140),
            Entry::new(irq_141),
            Entry::new(irq_142),
            Entry::new(irq_143),
            Entry::new(irq_144),
            Entry::new(irq_145),
            Entry::new(irq_146),
            Entry::new(irq_147),
            Entry::new(irq_148),
            Entry::new(irq_149),
            Entry::new(irq_150),
            Entry::new(irq_151),
            Entry::new(irq_152),
            Entry::new(irq_153),
            Entry::new(irq_154),
            Entry::new(irq_155),
            Entry::new(irq_156),
            Entry::new(irq_157),
            Entry::new(irq_158),
            Entry::new(irq_159),
            Entry::new(irq_160),
            Entry::new(irq_161),
            Entry::new(irq_162),
            Entry::new(irq_163),
            Entry::new(irq_164),
            Entry::new(irq_165),
            Entry::new(irq_166),
            Entry::new(irq_167),
            Entry::new(irq_168),
            Entry::new(irq_169),
            Entry::new(irq_170),
            Entry::new(irq_171),
            Entry::new(irq_172),
            Entry::new(irq_173),
            Entry::new(irq_174),
            Entry::new(irq_175),
            Entry::new(irq_176),
            Entry::new(irq_177),
            Entry::new(irq_178),
            Entry::new(irq_179),
            Entry::new(irq_180),
            Entry::new(irq_181),
            Entry::new(irq_182),
            Entry::new(irq_183),
            Entry::new(irq_184),
            Entry::new(irq_185),
            Entry::new(irq_186),
            Entry::new(irq_187),
            Entry::new(irq_188),
            Entry::new(irq_189),
            Entry::new(irq_190),
            Entry::new(irq_191),
            Entry::new(irq_192),
            Entry::new(irq_193),
            Entry::new(irq_194),
            Entry::new(irq_195),
            Entry::new(irq_196),
            Entry::new(irq_197),
            Entry::new(irq_198),
            Entry::new(irq_199),
            Entry::new(irq_200),
            Entry::new(irq_201),
            Entry::new(irq_202),
            Entry::new(irq_203),
            Entry::new(irq_204),
            Entry::new(irq_205),
            Entry::new(irq_206),
            Entry::new(irq_207),
            Entry::new(irq_208),
            Entry::new(irq_209),
            Entry::new(irq_210),
            Entry::new(irq_211),
            Entry::new(irq_212),
            Entry::new(irq_213),
            Entry::new(irq_214),
            Entry::new(irq_215),
            Entry::new(irq_216),
            Entry::new(irq_217),
            Entry::new(irq_218),
            Entry::new(irq_219),
            Entry::new(irq_220),
            Entry::new(irq_221),
            Entry::new(irq_222),
            Entry::new(irq_223),
            Entry::new(irq_224),
            Entry::new(irq_225),
            Entry::new(irq_226),
            Entry::new(irq_227),
            Entry::new(irq_228),
            Entry::new(irq_229),
            Entry::new(irq_230),
            Entry::new(irq_231),
            Entry::new(irq_232),
            Entry::new(irq_233),
            Entry::new(irq_234),
            Entry::new(irq_235),
            Entry::new(irq_236),
            Entry::new(irq_237),
            Entry::new(irq_238),
            Entry::new(irq_239),
            Entry::new(irq_240),
            Entry::new(irq_241),
            Entry::new(irq_242),
            Entry::new(irq_243),
            Entry::new(irq_244),
            Entry::new(irq_245),
            Entry::new(irq_246),
            Entry::new(irq_247),
            Entry::new(irq_248),
            Entry::new(irq_249),
            Entry::new(irq_250),
            Entry::new(irq_251),
            Entry::new(irq_252),
            Entry::new(irq_253),
            Entry::new(irq_254),
            Entry::new(irq_255),
        ],
    });

    let idt = IDT.get().unwrap();
    let idt_dtptr = crate::arch::x86_64::structures::DescriptorTablePointer {
        limit: u16::try_from(core::mem::size_of_val(idt) - 1).unwrap(),
        base: u64::try_from(core::ptr::from_ref(idt).addr()).unwrap(),
    };

    // Safety: The descriptor table pointer was properly constructed.
    unsafe {
        core::arch::asm!(
            "lidt [{}]",
            in(reg) &raw const idt_dtptr,
            options(readonly, nostack, preserves_flags)
        );
    }
}
