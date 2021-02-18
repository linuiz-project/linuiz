use x86_64::structures::idt::InterruptDescriptorTable;
pub use x86_64::structures::idt::InterruptStackFrame;

/* FAULT INTERRUPT HANDLERS */
extern "x86-interrupt" fn divide_error_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: SEGMENT NOT PRESENT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn debug_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: DEBUG\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn non_maskable_interrupt_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: NON-MASKABLE INTERRUPT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn breakpoint_handler(_stack_frame: &mut InterruptStackFrame) {
    // serialln!("CPU EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn overflow_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: OVERFLOW\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn bound_range_exceeded_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: BOUND RANGE EXCEEDED\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: INVALID OPCODE\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn device_not_available_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: DEVICE NOT AVAILABLE\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("CPU EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn invalid_tss_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "CPU EXCEPTION: INVALID TSS: {}\n{:#?}",
        error_code, stack_frame
    );
}

extern "x86-interrupt" fn segment_not_present_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "CPU EXCEPTION: SEGMENT NOT PRESENT: {}\n{:#?}",
        error_code, stack_frame
    );
}

extern "x86-interrupt" fn stack_segment_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "CPU EXCEPTION: STACK-SEGMENT FAULT: {}\n{:#?}",
        error_code, stack_frame
    );
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    #[repr(u8)]
    #[derive(Debug)]
    enum IndexType {
        GDT = 0b00,
        IDT0 = 0b01,
        LDT = 0b10,
        IDT1 = 0b11,
    }

    impl IndexType {
        fn from_u8(value: u8) -> Self {
            match value {
                x if x == (IndexType::GDT as u8) => IndexType::GDT,
                x if x == (IndexType::IDT0 as u8) => IndexType::IDT0,
                x if x == (IndexType::LDT as u8) => IndexType::LDT,
                x if x == (IndexType::IDT1 as u8) => IndexType::IDT1,
                _ => panic!("invalid selector index type!"),
            }
        }
    }

    let external = (error_code & 0x1) != 0x0;
    let selector_index_type = IndexType::from_u8(((error_code >> 1) & 0x3) as u8);
    let selector_index = (error_code >> 3) & 0x1FFF;

    panic!(
        "CPU EXCEPTION: GENERAL PROTECTION FAULT:\n External: {}\n IndexType: {:?}\n Index: {}\n {:#?}",
        external, selector_index_type, selector_index, stack_frame
    );
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    panic!(
        "CPU EXCEPTION: PAGE FAULT ({:?}): {:?}\n{:#?}",
        crate::registers::CR2::read(),
        error_code,
        stack_frame
    );
}

// --- reserved 15

extern "x86-interrupt" fn x87_floating_point_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: x87 FLOATING POINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn alignment_check_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "CPU EXCEPTION: ALIGNMENT CHECK: {}\n{:#?}",
        error_code, stack_frame
    );
}

// --- machine check (platform-specific, not required)

extern "x86-interrupt" fn simd_floating_point_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: SIMD FLOATING POINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn virtualization_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("CPU EXCEPTION: VIRTUALIZATION\n{:#?}", stack_frame);
}

// --- reserved 21-29

extern "x86-interrupt" fn security_exception_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "CPU EXCEPTION: SECURITY EXCEPTION: {}\n{:#?}",
        error_code, stack_frame
    );
}

// reserved 31

// --- triple fault (can't handle)

/* IDT */
static IDT: spin::Mutex<InterruptDescriptorTable> =
    spin::Mutex::new(InterruptDescriptorTable::new());

pub fn init() {
    let mut idt = IDT.lock();

    // fault interrupts
    idt.divide_error.set_handler_fn(divide_error_handler);
    idt.debug.set_handler_fn(debug_handler);
    idt.non_maskable_interrupt
        .set_handler_fn(non_maskable_interrupt_handler);
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.overflow.set_handler_fn(overflow_handler);
    idt.bound_range_exceeded
        .set_handler_fn(bound_range_exceeded_handler);
    idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
    idt.device_not_available
        .set_handler_fn(device_not_available_handler);
    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(crate::structures::gdt::DOUBLE_FAULT_IST_INDEX)
    };
    idt.invalid_tss.set_handler_fn(invalid_tss_handler);
    idt.segment_not_present
        .set_handler_fn(segment_not_present_handler);
    idt.stack_segment_fault
        .set_handler_fn(stack_segment_handler);
    idt.general_protection_fault
        .set_handler_fn(general_protection_fault_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    // --- reserved 15
    idt.x87_floating_point
        .set_handler_fn(x87_floating_point_handler);
    idt.alignment_check.set_handler_fn(alignment_check_handler);
    // --- machine check (platform specific, not required)
    idt.simd_floating_point
        .set_handler_fn(simd_floating_point_handler);
    idt.virtualization.set_handler_fn(virtualization_handler);
    // --- reserved 21-29
    idt.security_exception
        .set_handler_fn(security_exception_handler);
    // --- triple fault (can't handle)

    unsafe { idt.load_unsafe() };
}

pub fn set_interrupt_handler(
    index: u8,
    handler: extern "x86-interrupt" fn(&mut InterruptStackFrame),
) {
    crate::instructions::interrupts::without_interrupts(|| {
        if index >= 32 {
            trace!("Modifying IDT handler at index: {}", index);
            IDT.lock()[index as usize].set_handler_fn(handler);
        } else {
            panic!("interrupt handler index must be >=32 (0..32 are reserved)");
        }
    });
}
