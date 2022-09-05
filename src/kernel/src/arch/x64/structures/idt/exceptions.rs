use crate::interrupts::get_common_exception_handler;
use libkernel::{Address, Virtual};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode, SelectorErrorCode};
/// x64 exception wrapper type.
#[repr(C, u8)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum Exception {
    /// Generated upon an attempt to divide by zero.
    DivideError(InterruptStackFrame),

    /// Exception generated due to various conditions, outlined within the IA-32 SDM.
    /// Debug registers will be updated to provide context to this exception.
    Debug(InterruptStackFrame),

    /// Typically caused by unrecoverable RAM or other hardware errors.
    NonMaskable(InterruptStackFrame),

    /// Occurs when `int3` is called in software.
    Breakpoint(InterruptStackFrame),

    /// Occurs when the `into` instruction is executed with the `OVERFLOW` bit set in RFlags.
    Overflow(InterruptStackFrame),

    /// Occurs when the `bound` instruction is executed and fails its check.
    BoundRangeExceeded(InterruptStackFrame),

    /// Occurs when the processor tries to execute an invalid or undefined opcode.
    InvalidOpcode(InterruptStackFrame),

    /// Generated when there is no FPU available, but an FPU-reliant instruction is executed.
    DeviceNotAvailable(InterruptStackFrame),

    /// Occurs when an exception is unhandled or when an exception occurs while the CPU is
    /// trying to call an exception handler.
    DoubleFault(InterruptStackFrame),

    /// Occurs when an invalid segment selector is referenced as part of a task switch, or as a
    /// result of a control transfer through a gate descriptor, which results in an invalid
    /// stack-segment reference using an SS selector in the TSS
    InvalidTSS(InterruptStackFrame, SelectorErrorCode),

    /// Occurs when trying to load a segment or gate which has its `PRESENT` bit unset.
    SegmentNotPresent(InterruptStackFrame, SelectorErrorCode),

    /// Occurs when:
    ///     - Loading a stack-segment referencing a segment descriptor which is not present;
    ///     - Any `push`/`pop` instruction or any instruction using `esp`/`ebp` as a base register
    ///         is executed, while the stack address is not in canonical form;
    ///     - The stack-limit check fails.
    StackSegmentFault(InterruptStackFrame, SelectorErrorCode),

    /// Occurs when:
    ///     - Segment error (privilege, type, limit, r/w rights).
    ///     - Executing a privileged instruction while CPL isn't supervisor (CPL0)
    ///     - Writing a `1` in a reserved register field or writing invalid value combinations (e.g. `CR0` with `PE` unset and `PG` set).
    ///     - Referencing or accessing a null descriptor.
    GeneralProtectionFault(InterruptStackFrame, SelectorErrorCode),

    /// Occurs when:
    ///     - A page directory or table entry is not present in physical memory.
    ///     - Attempting to load the instruction TLB with a translation for a non-executable page.
    ///     - A protection cehck (privilege, r/w) failed.
    ///     - A reserved bit in the page directory table or entries is set to 1.
    PageFault(InterruptStackFrame, PageFaultErrorCode, Address<Virtual>),

    /// Occurs when the `fwait` or `wait` instruction (or any floating point instruction) is executed, and the
    /// following conditions are true:
    ///     - `CR0.NE` is set.
    ///     - An unmasked x87 floating point exception is pending (i.e. the exception bit in the x87 floating point status-word register is set).
    x87FloatingPoint(InterruptStackFrame),

    /// Occurs when alignment checking is enabled and an unaligned memory data reference is performed.
    ///
    /// REMARK: Alignment checks are only performed when in usermode (CPL3).
    AlignmentCheck(InterruptStackFrame, u64),

    /// Exception is model-specific and processor implementations are not required to support it.
    ///
    /// REMARK: It uses model-specific registers (MSRs) to provide error information.
    ///         It is disabled by default. Set `CR4.MCE` to enable it.
    MachineCheck(InterruptStackFrame),

    /* VIRTUALIZATION EXCEPTIONS (not supported) */
    /// Occurs when an unmasked 128-bit media floating-point exception occurs and the `CR4.OSXMMEXCPT` bit
    /// is set. If it is not set, this error condition will trigger an invalid opcode exception instead.
    SIMDFlaotingPoint(InterruptStackFrame),

    /// Occurs only on processors that support setting the `EPT-violation` bit for VM execution control.
    Virtualization(InterruptStackFrame),

    /// Occurs under several conditions on the `ret`/`iret`/`rstorssp`/`setssbsy` instructions.
    ControlProtection(InterruptStackFrame),

    HypervisorInjection(InterruptStackFrame),

    VMMCommunication(InterruptStackFrame),

    Security(InterruptStackFrame, u64),

    /// Not an exception; it will never be handled by an interrupt handler. It is included here for completeness.
    TripleFault,
}

impl Exception {
    pub fn common_exception_handler(exception: Self) {
        match exception {
            Self::PageFault(_, _, address) => {
                use crate::memory::PageAttributes;
                use libkernel::memory::Page;

                let kernel_frame_manager = crate::memory::get_kernel_frame_manager();
                // SAFETY: Kernel HHDM is guaranteed by the kernel to be valid.
                let page_manager = unsafe {
                    crate::memory::PageManager::from_current(
                        &Page::from_address(crate::memory::get_kernel_hhdm_address()).unwrap(),
                    )
                };

                // Determine if the page fault occured within a demand-paged page.
                let fault_page = Page::from_address_contains(address);
                if  let Some(mut fault_page_attributes) = page_manager.get_page_attributes(&fault_page)
                    && fault_page_attributes.contains(PageAttributes::DEMAND) {
                        page_manager.auto_map(
                            &fault_page,
                            {
                                // remove demand bit ...
                                fault_page_attributes.remove(PageAttributes::DEMAND);
                                // ... insert usable RW bits ...
                                fault_page_attributes.insert(PageAttributes::RW);
                                // ... return attributes
                                fault_page_attributes
                            },
                            kernel_frame_manager
                        );

                        // SAFETY: We know the page was just mapped, and contains no relevant memory.
                        unsafe { fault_page.clear_memory() };
            }
            }

            exception => panic!("{:#?}", exception),
        }
    }
}

/* FAULT INTERRUPT HANDLERS */
extern "x86-interrupt" fn divide_error_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::DivideError(stack_frame))
}

extern "x86-interrupt" fn debug_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::Debug(stack_frame))
}

extern "x86-interrupt" fn non_maskable_interrupt_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::NonMaskable(stack_frame))
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::Breakpoint(stack_frame))
}

extern "x86-interrupt" fn overflow_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::Overflow(stack_frame))
}

extern "x86-interrupt" fn bound_range_exceeded_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::BoundRangeExceeded(stack_frame))
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::InvalidOpcode(stack_frame))
}

extern "x86-interrupt" fn device_not_available_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::DeviceNotAvailable(stack_frame))
}

extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, _: u64) -> ! {
    get_common_exception_handler()(Exception::DoubleFault(stack_frame));
    // Wait indefinite in case the above exception handler returns control flow.
    crate::interrupts::wait_loop()
}

extern "x86-interrupt" fn invalid_tss_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    get_common_exception_handler()(Exception::InvalidTSS(stack_frame, SelectorErrorCode::new_truncate(error_code)))
}

extern "x86-interrupt" fn segment_not_present_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    get_common_exception_handler()(Exception::SegmentNotPresent(
        stack_frame,
        SelectorErrorCode::new_truncate(error_code),
    ))
}

extern "x86-interrupt" fn stack_segment_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    get_common_exception_handler()(Exception::StackSegmentFault(
        stack_frame,
        SelectorErrorCode::new_truncate(error_code),
    ))
}

extern "x86-interrupt" fn general_protection_fault_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    get_common_exception_handler()(Exception::GeneralProtectionFault(
        stack_frame,
        SelectorErrorCode::new_truncate(error_code),
    ))
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    get_common_exception_handler()(Exception::PageFault(
        stack_frame,
        error_code,
        crate::arch::x64::registers::control::CR2::read(),
    ))
}

// --- reserved 15

extern "x86-interrupt" fn x87_floating_point_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::x87FloatingPoint(stack_frame))
}

extern "x86-interrupt" fn alignment_check_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    get_common_exception_handler()(Exception::AlignmentCheck(stack_frame, error_code))
}

extern "x86-interrupt" fn machine_check_handler(stack_frame: InterruptStackFrame) -> ! {
    get_common_exception_handler()(Exception::MachineCheck(stack_frame));
    // Wait indefinite in case the above exception handler returns control flow.
    crate::interrupts::wait_loop()
}

extern "x86-interrupt" fn simd_floating_point_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::SIMDFlaotingPoint(stack_frame))
}

extern "x86-interrupt" fn virtualization_handler(stack_frame: InterruptStackFrame) {
    get_common_exception_handler()(Exception::Virtualization(stack_frame))
}

// --- reserved 21-29

extern "x86-interrupt" fn security_exception_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    get_common_exception_handler()(Exception::Security(stack_frame, error_code))
}

// reserved 31
// --- triple fault (can't handle)

/// Defines set indexes which specified interrupts will use for stacks.
#[repr(usize)]
#[derive(Debug, Clone, Copy)]
pub enum StackTableIndex {
    Debug = 0,
    NonMaskable = 1,
    DoubleFault = 2,
    MachineCheck = 3,
}

pub fn set_exception_handlers(idt: &mut InterruptDescriptorTable) {
    unsafe {
        idt.debug.set_handler_fn(debug_handler).set_stack_index(StackTableIndex::Debug as u16);
        idt.non_maskable_interrupt
            .set_handler_fn(non_maskable_interrupt_handler)
            .set_stack_index(StackTableIndex::NonMaskable as u16);
        idt.double_fault.set_handler_fn(double_fault_handler).set_stack_index(StackTableIndex::DoubleFault as u16);
        idt.machine_check.set_handler_fn(machine_check_handler).set_stack_index(StackTableIndex::MachineCheck as u16);
    }

    idt.divide_error.set_handler_fn(divide_error_handler);
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.overflow.set_handler_fn(overflow_handler);
    idt.bound_range_exceeded.set_handler_fn(bound_range_exceeded_handler);
    idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
    idt.device_not_available.set_handler_fn(device_not_available_handler);
    idt.invalid_tss.set_handler_fn(invalid_tss_handler);
    idt.segment_not_present.set_handler_fn(segment_not_present_handler);
    idt.stack_segment_fault.set_handler_fn(stack_segment_handler);
    idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    // --- reserved 15
    idt.x87_floating_point.set_handler_fn(x87_floating_point_handler);
    idt.alignment_check.set_handler_fn(alignment_check_handler);
    idt.simd_floating_point.set_handler_fn(simd_floating_point_handler);
    idt.virtualization.set_handler_fn(virtualization_handler);
    // --- reserved 21-29
    idt.security_exception.set_handler_fn(security_exception_handler);
    // --- triple fault (can't handle)
}
