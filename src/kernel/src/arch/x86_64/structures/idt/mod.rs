#![allow(unused_unsafe)]

mod entry;
use entry::*;

mod stubs;
use stubs::*;

mod isf;
pub use isf::*;

mod error_codes;
pub use error_codes::*;

use crate::arch::x86_64::structures::{DescriptorTablePointer, tss::InterruptStackTableIndex};

crate::singleton! {
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
    pub InterruptDescriptorTable {
        /// A divide error (`#DE`) occurs when the denominator of a DIV instruction or
        /// an IDIV instruction is 0. A `#DE` also occurs if the result is too large to be
        /// represented in the destination.
        ///
        /// The saved instruction pointer points to the instruction that caused the `#DE`.
        ///
        /// The vector number of the `#DE` exception is 0.
        divide_error: Entry,

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
        debug: Entry,

        /// An non maskable interrupt exception (NMI) occurs as a result of system logic
        /// signaling a non-maskable interrupt to the processor.
        ///
        /// The processor recognizes an NMI at an instruction boundary.
        /// The saved instruction pointer points to the instruction immediately following the
        /// boundary where the NMI was recognized.
        ///
        /// The vector number of the NMI exception is 2.
        non_maskable_interrupt: Entry,

        /// A breakpoint (`#BP`) exception occurs when an `INT3` instruction is executed. The
        /// `INT3` is normally used by debug software to set instruction breakpoints by replacing
        ///
        /// The saved instruction pointer points to the byte after the `INT3` instruction.
        ///
        /// The vector number of the `#BP` exception is 3.
        breakpoint: Entry,

        /// An overflow exception (`#OF`) occurs as a result of executing an `INTO` instruction
        /// while the overflow bit in `RFLAGS` is set to 1.
        ///
        /// The saved instruction pointer points to the instruction following the `INTO`
        /// instruction that caused the `#OF`.
        ///
        /// The vector number of the `#OF` exception is 4.
        overflow: Entry,

        /// A bound-range exception (`#BR`) exception can occur as a result of executing
        /// the `BOUND` instruction. The `BOUND` instruction compares an array index (first
        /// operand) with the lower bounds and upper bounds of an array (second operand).
        /// If the array index is not within the array boundary, the `#BR` occurs.
        ///
        /// The saved instruction pointer points to the `BOUND` instruction that caused the `#BR`.
        ///
        /// The vector number of the `#BR` exception is 5.
        bound_range_exceeded: Entry,

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
        invalid_opcode: Entry,

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
        device_not_available: Entry,

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
        double_fault: Entry,

        /// This interrupt vector is reserved. It is for a discontinued exception originally used
        /// by processors that supported external x87-instruction coprocessors. On those processors,
        /// the exception condition is caused by an invalid-segment or invalid-page access on an
        /// x87-instruction coprocessor-instruction operand. On current processors, this condition
        /// causes a general-protection exception to occur.
        coprocessor_segment_overrun: Entry,

        /// An invalid TSS exception (`#TS`) occurs only as a result of a control transfer through
        /// a gate descriptor that results in an invalid stack-segment reference using an `SS`
        /// selector in the TSS.
        ///
        /// The returned error code is the `SS` segment selector. The saved instruction pointer
        /// points to the control-transfer instruction that caused the `#TS`.
        ///
        /// The vector number of the `#TS` exception is 10.
        invalid_tss: Entry,

        /// An segment-not-present exception (`#NP`) occurs when an attempt is made to load a
        /// segment or gate with a clear present bit.
        ///
        /// The returned error code is the segment-selector index of the segment descriptor
        /// causing the `#NP` exception. The saved instruction pointer points to the instruction
        /// that loaded the segment selector resulting in the `#NP`.
        ///
        /// The vector number of the `#NP` exception is 11.
        segment_not_present: Entry,

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
        stack_segment_fault: Entry,

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
        general_protection_fault: Entry,

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
        page_fault: Entry,

        /// vector nr. 15
        _1: [Entry; 1],

        /// The x87 Floating-Point Exception-Pending exception (`#MF`) is used to handle unmasked x87
        /// floating-point exceptions. In 64-bit mode, the x87 floating point unit is not used
        /// anymore, so this exception is only relevant when executing programs in the 32-bit
        /// compatibility mode.
        ///
        /// The vector number of the `#MF` exception is 16.
        x87_floating_point: Entry,

        /// An alignment check exception (`#AC`) occurs when an unaligned-memory data reference
        /// is performed while alignment checking is enabled. An `#AC` can occur only when CPL=3.
        ///
        /// The returned error code is always zero. The saved instruction pointer points to the
        /// instruction that caused the `#AC`.
        ///
        /// The vector number of the `#AC` exception is 17.
        alignment_check: Entry,

        /// The machine check exception (`#MC`) is model specific. Processor implementations
        /// are not required to support the `#MC` exception, and those implementations that do
        /// support `#MC` can vary in how the `#MC` exception mechanism works.
        ///
        /// There is no reliable way to restart the program.
        ///
        /// The vector number of the `#MC` exception is 18.
        machine_check: Entry,

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
        simd_floating_point: Entry,

        /// vector nr. 20
        virtualization: Entry,

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
        cp_protection_exception: Entry,

        /// vector nr. 22-27
        _2: [Entry; 6],

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
        hv_injection_exception: Entry,

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
        vmm_communication_exception: Entry,

        /// The Security Exception (`#SX`) signals security-sensitive events that occur while
        /// executing the VMM, in the form of an exception so that the VMM may take appropriate
        /// action. (A VMM would typically intercept comparable sensitive events in the guest.)
        /// In the current implementation, the only use of the `#SX` is to redirect external INITs
        /// into an exception so that the VMM may — among other possibilities.
        ///
        /// The only error code currently defined is 1, and indicates redirection of INIT has occurred.
        ///
        /// The vector number of the ``#SX`` exception is 30.
        security_exception: Entry,

        /// vector nr. 31
        _3: [Entry; 1],

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
        interrupts: [Entry; 224],
    }

    fn init() {
        // Safety:
        //  - All function addresses are correctly set to linked interrupt stubs.
        //  - Entries with specified stack table indexes are set correctly.
        //  - Entries with specified privilege levels are set correctly.
        unsafe {
            Self {
                divide_error: Entry::new(__de_stub.as_usize()),
                // Safety: Stack table index is set to `Debug` stack.
                debug: Entry::new_with_stack(
                    __db_stub.as_usize(),
                    InterruptStackTableIndex::Debug,
                ),
                // Safety: Stack table index is set to `NonMaskableInterrupt` stack.
                non_maskable_interrupt: Entry::new_with_stack(
                    __nm_stub.as_usize(),
                    InterruptStackTableIndex::NonMaskableInterrupt,
                ),
                breakpoint: Entry::new(__bp_stub.as_usize()),
                overflow: Entry::new(__of_stub.as_usize()),
                bound_range_exceeded: Entry::new(__br_stub.as_usize()),
                invalid_opcode: Entry::new(__ud_stub.as_usize()),
                device_not_available: Entry::new(__na_stub.as_usize()),
                // Safety: Stack table index is set to `DoubleFault` stack.
                double_fault: Entry::new_with_stack(
                    __df_stub.as_usize(),
                    InterruptStackTableIndex::DoubleFault,
                ),
                coprocessor_segment_overrun: Entry::missing(),
                invalid_tss: Entry::new(__ts_stub.as_usize()),
                segment_not_present: Entry::new(__np_stub.as_usize()),
                stack_segment_fault: Entry::new(__ss_stub.as_usize()),
                general_protection_fault: Entry::new(__gp_stub.as_usize()),
                page_fault: Entry::new(__pf_stub.as_usize()),
                _1: [Entry::missing(); _],
                x87_floating_point: Entry::new(__mf_stub.as_usize()),
                alignment_check: Entry::new(__ac_stub.as_usize()),
                // Safety: Stack table index is set to `MachineCheck` stack.
                machine_check: unsafe {
                    Entry::new_with_stack(
                        __mc_stub.as_usize(),
                        InterruptStackTableIndex::MachineCheck,
                    )
                },
                simd_floating_point: Entry::new(__xm_stub.as_usize()),
                virtualization: Entry::new(__ve_stub.as_usize()),
                cp_protection_exception: Entry::missing(),
                _2: [Entry::missing(); _],
                hv_injection_exception: Entry::missing(),
                vmm_communication_exception: Entry::missing(),
                security_exception: Entry::missing(),
                _3: [Entry::missing(); _],
                interrupts: [
                    // Safety: Privilege level is set for coming FROM userspace (ring 3) for syscalls.
                    unsafe {
                        Entry::new_with_privilege(
                            __irq_128_stub.as_usize(),
                            super::gdt::PrivilegeLevel::Ring3,
                        )
                    },
                    Entry::new(__irq_32_stub.as_usize()),
                    Entry::new(__irq_33_stub.as_usize()),
                    Entry::new(__irq_34_stub.as_usize()),
                    Entry::new(__irq_35_stub.as_usize()),
                    Entry::new(__irq_36_stub.as_usize()),
                    Entry::new(__irq_37_stub.as_usize()),
                    Entry::new(__irq_39_stub.as_usize()),
                    Entry::new(__irq_38_stub.as_usize()),
                    Entry::new(__irq_40_stub.as_usize()),
                    Entry::new(__irq_41_stub.as_usize()),
                    Entry::new(__irq_42_stub.as_usize()),
                    Entry::new(__irq_43_stub.as_usize()),
                    Entry::new(__irq_44_stub.as_usize()),
                    Entry::new(__irq_45_stub.as_usize()),
                    Entry::new(__irq_46_stub.as_usize()),
                    Entry::new(__irq_47_stub.as_usize()),
                    Entry::new(__irq_48_stub.as_usize()),
                    Entry::new(__irq_49_stub.as_usize()),
                    Entry::new(__irq_50_stub.as_usize()),
                    Entry::new(__irq_51_stub.as_usize()),
                    Entry::new(__irq_52_stub.as_usize()),
                    Entry::new(__irq_53_stub.as_usize()),
                    Entry::new(__irq_54_stub.as_usize()),
                    Entry::new(__irq_55_stub.as_usize()),
                    Entry::new(__irq_56_stub.as_usize()),
                    Entry::new(__irq_57_stub.as_usize()),
                    Entry::new(__irq_58_stub.as_usize()),
                    Entry::new(__irq_59_stub.as_usize()),
                    Entry::new(__irq_60_stub.as_usize()),
                    Entry::new(__irq_61_stub.as_usize()),
                    Entry::new(__irq_62_stub.as_usize()),
                    Entry::new(__irq_63_stub.as_usize()),
                    Entry::new(__irq_64_stub.as_usize()),
                    Entry::new(__irq_65_stub.as_usize()),
                    Entry::new(__irq_66_stub.as_usize()),
                    Entry::new(__irq_67_stub.as_usize()),
                    Entry::new(__irq_68_stub.as_usize()),
                    Entry::new(__irq_69_stub.as_usize()),
                    Entry::new(__irq_70_stub.as_usize()),
                    Entry::new(__irq_71_stub.as_usize()),
                    Entry::new(__irq_72_stub.as_usize()),
                    Entry::new(__irq_73_stub.as_usize()),
                    Entry::new(__irq_74_stub.as_usize()),
                    Entry::new(__irq_75_stub.as_usize()),
                    Entry::new(__irq_76_stub.as_usize()),
                    Entry::new(__irq_77_stub.as_usize()),
                    Entry::new(__irq_78_stub.as_usize()),
                    Entry::new(__irq_79_stub.as_usize()),
                    Entry::new(__irq_80_stub.as_usize()),
                    Entry::new(__irq_81_stub.as_usize()),
                    Entry::new(__irq_82_stub.as_usize()),
                    Entry::new(__irq_83_stub.as_usize()),
                    Entry::new(__irq_84_stub.as_usize()),
                    Entry::new(__irq_85_stub.as_usize()),
                    Entry::new(__irq_86_stub.as_usize()),
                    Entry::new(__irq_87_stub.as_usize()),
                    Entry::new(__irq_88_stub.as_usize()),
                    Entry::new(__irq_89_stub.as_usize()),
                    Entry::new(__irq_90_stub.as_usize()),
                    Entry::new(__irq_91_stub.as_usize()),
                    Entry::new(__irq_92_stub.as_usize()),
                    Entry::new(__irq_93_stub.as_usize()),
                    Entry::new(__irq_94_stub.as_usize()),
                    Entry::new(__irq_95_stub.as_usize()),
                    Entry::new(__irq_96_stub.as_usize()),
                    Entry::new(__irq_97_stub.as_usize()),
                    Entry::new(__irq_98_stub.as_usize()),
                    Entry::new(__irq_99_stub.as_usize()),
                    Entry::new(__irq_100_stub.as_usize()),
                    Entry::new(__irq_101_stub.as_usize()),
                    Entry::new(__irq_102_stub.as_usize()),
                    Entry::new(__irq_103_stub.as_usize()),
                    Entry::new(__irq_104_stub.as_usize()),
                    Entry::new(__irq_105_stub.as_usize()),
                    Entry::new(__irq_106_stub.as_usize()),
                    Entry::new(__irq_107_stub.as_usize()),
                    Entry::new(__irq_108_stub.as_usize()),
                    Entry::new(__irq_109_stub.as_usize()),
                    Entry::new(__irq_110_stub.as_usize()),
                    Entry::new(__irq_111_stub.as_usize()),
                    Entry::new(__irq_112_stub.as_usize()),
                    Entry::new(__irq_113_stub.as_usize()),
                    Entry::new(__irq_114_stub.as_usize()),
                    Entry::new(__irq_115_stub.as_usize()),
                    Entry::new(__irq_116_stub.as_usize()),
                    Entry::new(__irq_117_stub.as_usize()),
                    Entry::new(__irq_118_stub.as_usize()),
                    Entry::new(__irq_119_stub.as_usize()),
                    Entry::new(__irq_120_stub.as_usize()),
                    Entry::new(__irq_121_stub.as_usize()),
                    Entry::new(__irq_122_stub.as_usize()),
                    Entry::new(__irq_123_stub.as_usize()),
                    Entry::new(__irq_124_stub.as_usize()),
                    Entry::new(__irq_125_stub.as_usize()),
                    Entry::new(__irq_126_stub.as_usize()),
                    Entry::new(__irq_127_stub.as_usize()),
                    Entry::new(__irq_129_stub.as_usize()),
                    Entry::new(__irq_130_stub.as_usize()),
                    Entry::new(__irq_131_stub.as_usize()),
                    Entry::new(__irq_132_stub.as_usize()),
                    Entry::new(__irq_133_stub.as_usize()),
                    Entry::new(__irq_134_stub.as_usize()),
                    Entry::new(__irq_135_stub.as_usize()),
                    Entry::new(__irq_136_stub.as_usize()),
                    Entry::new(__irq_137_stub.as_usize()),
                    Entry::new(__irq_138_stub.as_usize()),
                    Entry::new(__irq_139_stub.as_usize()),
                    Entry::new(__irq_140_stub.as_usize()),
                    Entry::new(__irq_141_stub.as_usize()),
                    Entry::new(__irq_142_stub.as_usize()),
                    Entry::new(__irq_143_stub.as_usize()),
                    Entry::new(__irq_144_stub.as_usize()),
                    Entry::new(__irq_145_stub.as_usize()),
                    Entry::new(__irq_146_stub.as_usize()),
                    Entry::new(__irq_147_stub.as_usize()),
                    Entry::new(__irq_148_stub.as_usize()),
                    Entry::new(__irq_149_stub.as_usize()),
                    Entry::new(__irq_150_stub.as_usize()),
                    Entry::new(__irq_151_stub.as_usize()),
                    Entry::new(__irq_152_stub.as_usize()),
                    Entry::new(__irq_153_stub.as_usize()),
                    Entry::new(__irq_154_stub.as_usize()),
                    Entry::new(__irq_155_stub.as_usize()),
                    Entry::new(__irq_156_stub.as_usize()),
                    Entry::new(__irq_157_stub.as_usize()),
                    Entry::new(__irq_158_stub.as_usize()),
                    Entry::new(__irq_159_stub.as_usize()),
                    Entry::new(__irq_160_stub.as_usize()),
                    Entry::new(__irq_161_stub.as_usize()),
                    Entry::new(__irq_162_stub.as_usize()),
                    Entry::new(__irq_163_stub.as_usize()),
                    Entry::new(__irq_164_stub.as_usize()),
                    Entry::new(__irq_165_stub.as_usize()),
                    Entry::new(__irq_166_stub.as_usize()),
                    Entry::new(__irq_167_stub.as_usize()),
                    Entry::new(__irq_168_stub.as_usize()),
                    Entry::new(__irq_169_stub.as_usize()),
                    Entry::new(__irq_170_stub.as_usize()),
                    Entry::new(__irq_171_stub.as_usize()),
                    Entry::new(__irq_172_stub.as_usize()),
                    Entry::new(__irq_173_stub.as_usize()),
                    Entry::new(__irq_174_stub.as_usize()),
                    Entry::new(__irq_175_stub.as_usize()),
                    Entry::new(__irq_176_stub.as_usize()),
                    Entry::new(__irq_177_stub.as_usize()),
                    Entry::new(__irq_178_stub.as_usize()),
                    Entry::new(__irq_179_stub.as_usize()),
                    Entry::new(__irq_180_stub.as_usize()),
                    Entry::new(__irq_181_stub.as_usize()),
                    Entry::new(__irq_182_stub.as_usize()),
                    Entry::new(__irq_183_stub.as_usize()),
                    Entry::new(__irq_184_stub.as_usize()),
                    Entry::new(__irq_185_stub.as_usize()),
                    Entry::new(__irq_186_stub.as_usize()),
                    Entry::new(__irq_187_stub.as_usize()),
                    Entry::new(__irq_188_stub.as_usize()),
                    Entry::new(__irq_189_stub.as_usize()),
                    Entry::new(__irq_190_stub.as_usize()),
                    Entry::new(__irq_191_stub.as_usize()),
                    Entry::new(__irq_192_stub.as_usize()),
                    Entry::new(__irq_193_stub.as_usize()),
                    Entry::new(__irq_194_stub.as_usize()),
                    Entry::new(__irq_195_stub.as_usize()),
                    Entry::new(__irq_196_stub.as_usize()),
                    Entry::new(__irq_197_stub.as_usize()),
                    Entry::new(__irq_198_stub.as_usize()),
                    Entry::new(__irq_199_stub.as_usize()),
                    Entry::new(__irq_200_stub.as_usize()),
                    Entry::new(__irq_201_stub.as_usize()),
                    Entry::new(__irq_202_stub.as_usize()),
                    Entry::new(__irq_203_stub.as_usize()),
                    Entry::new(__irq_204_stub.as_usize()),
                    Entry::new(__irq_205_stub.as_usize()),
                    Entry::new(__irq_206_stub.as_usize()),
                    Entry::new(__irq_207_stub.as_usize()),
                    Entry::new(__irq_208_stub.as_usize()),
                    Entry::new(__irq_209_stub.as_usize()),
                    Entry::new(__irq_210_stub.as_usize()),
                    Entry::new(__irq_211_stub.as_usize()),
                    Entry::new(__irq_212_stub.as_usize()),
                    Entry::new(__irq_213_stub.as_usize()),
                    Entry::new(__irq_214_stub.as_usize()),
                    Entry::new(__irq_215_stub.as_usize()),
                    Entry::new(__irq_216_stub.as_usize()),
                    Entry::new(__irq_217_stub.as_usize()),
                    Entry::new(__irq_218_stub.as_usize()),
                    Entry::new(__irq_219_stub.as_usize()),
                    Entry::new(__irq_220_stub.as_usize()),
                    Entry::new(__irq_221_stub.as_usize()),
                    Entry::new(__irq_222_stub.as_usize()),
                    Entry::new(__irq_223_stub.as_usize()),
                    Entry::new(__irq_224_stub.as_usize()),
                    Entry::new(__irq_225_stub.as_usize()),
                    Entry::new(__irq_226_stub.as_usize()),
                    Entry::new(__irq_227_stub.as_usize()),
                    Entry::new(__irq_228_stub.as_usize()),
                    Entry::new(__irq_229_stub.as_usize()),
                    Entry::new(__irq_230_stub.as_usize()),
                    Entry::new(__irq_231_stub.as_usize()),
                    Entry::new(__irq_232_stub.as_usize()),
                    Entry::new(__irq_233_stub.as_usize()),
                    Entry::new(__irq_234_stub.as_usize()),
                    Entry::new(__irq_235_stub.as_usize()),
                    Entry::new(__irq_236_stub.as_usize()),
                    Entry::new(__irq_237_stub.as_usize()),
                    Entry::new(__irq_238_stub.as_usize()),
                    Entry::new(__irq_239_stub.as_usize()),
                    Entry::new(__irq_240_stub.as_usize()),
                    Entry::new(__irq_241_stub.as_usize()),
                    Entry::new(__irq_242_stub.as_usize()),
                    Entry::new(__irq_243_stub.as_usize()),
                    Entry::new(__irq_244_stub.as_usize()),
                    Entry::new(__irq_245_stub.as_usize()),
                    Entry::new(__irq_246_stub.as_usize()),
                    Entry::new(__irq_247_stub.as_usize()),
                    Entry::new(__irq_248_stub.as_usize()),
                    Entry::new(__irq_249_stub.as_usize()),
                    Entry::new(__irq_250_stub.as_usize()),
                    Entry::new(__irq_251_stub.as_usize()),
                    Entry::new(__irq_252_stub.as_usize()),
                    Entry::new(__irq_253_stub.as_usize()),
                    Entry::new(__irq_254_stub.as_usize()),
                    Entry::new(__irq_255_stub.as_usize()),
                ],
            }
        }
    }
}

impl core::ops::Index<u8> for InterruptDescriptorTable {
    type Output = Entry;

    /// Returns the IDT entry with the specified index.
    ///
    /// Panics if the entry is an exception that pushes an error code (use the struct fields for accessing these entries).
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
    fn index_mut(&mut self, index: u8) -> &mut Self::Output {
        match index {
            index @ 32..=255 => &mut self.interrupts[usize::from(index) - 32],
            index => panic!("Exception vector '{index}' must be directly indexed."),
        }
    }
}

impl InterruptDescriptorTable {
    pub fn load_static() {
        let idt = Self::get_static();

        let dtptr = DescriptorTablePointer::from(idt);

        trace!("Loading: {:X?}:\n{dtptr:#X?}", core::ptr::from_ref(idt));

        // Safety: The descriptor table pointer was properly constructed.
        unsafe {
            core::arch::asm!(
                "lidt [{}]",
                in(reg) &raw const dtptr,
                options(readonly, nostack, preserves_flags)
            );
        }
    }
}
