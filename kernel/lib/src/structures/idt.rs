use x86_64::structures::idt::InterruptDescriptorTable;
pub use x86_64::structures::idt::InterruptStackFrame;

/* FAULT INTERRUPT HANDLERS */
extern "x86-interrupt" fn divide_error_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: SEGMENT NOT PRESENT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn debug_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: DEBUG\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn non_maskable_interrupt_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: NON-MASKABLE INTERRUPT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn overflow_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: OVERFLOW\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn bound_range_exceeded_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: BOUND RANGE EXCEEDED\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: INVALID OPCODE\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn device_not_available_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: DEVICE NOT AVAILABLE\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("CPU EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn invalid_tss_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    panic!(
        "CPU EXCEPTION: INVALID TSS: {}\n{:#?}",
        error_code, stack_frame
    );
}

extern "x86-interrupt" fn segment_not_present_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    use bit_field::BitField;

    #[derive(Debug)]
    enum StackSelectorErrorTable {
        GDT,
        IDT,
        LDT,
        Invalid(u32),
    }

    struct StackSelectorErrorCode(u32);
    impl core::fmt::Debug for StackSelectorErrorCode {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter
                .debug_struct("Stack Selector Error")
                .field("External", &self.0.get_bit(0))
                .field(
                    "Table",
                    &match self.0.get_bits(1..3) {
                        0b00 => StackSelectorErrorTable::GDT,
                        0b01 | 0b11 => StackSelectorErrorTable::IDT,
                        0b10 => StackSelectorErrorTable::LDT,
                        value => StackSelectorErrorTable::Invalid(value),
                    },
                )
                .field(
                    "Index",
                    &format_args!("0x{0:X} | {0}", self.0.get_bits(3..16)),
                )
                .finish()
        }
    }

    panic!(
        "CPU EXCEPTION: SEGMENT NOT PRESENT:\nError: {:?}\n{:#?}",
        StackSelectorErrorCode(error_code as u32),
        stack_frame
    );
}

extern "x86-interrupt" fn stack_segment_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    panic!(
        "CPU EXCEPTION: STACK-SEGMENT FAULT: {}\n{:#?}",
        error_code, stack_frame
    );
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
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
        "CPU EXCEPTION: GENERAL PROTECTION FAULT:\n External: {}\n IndexType: {:?}\n Index: 0x{:X}\n {:#?}",
        external, selector_index_type, selector_index, stack_frame
    );
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    panic!(
        "CPU EXCEPTION: PAGE FAULT ({:?}): {:?}\n{:#?}",
        crate::registers::control::CR2::read(),
        error_code.bits(),
        stack_frame
    );
}

// --- reserved 15

extern "x86-interrupt" fn x87_floating_point_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: x87 FLOATING POINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn alignment_check_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "CPU EXCEPTION: ALIGNMENT CHECK: {}\n{:#?}",
        error_code, stack_frame
    );
}

// --- machine check (platform-specific, not required)

extern "x86-interrupt" fn simd_floating_point_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: SIMD FLOATING POINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn virtualization_handler(stack_frame: InterruptStackFrame) {
    panic!("CPU EXCEPTION: VIRTUALIZATION\n{:#?}", stack_frame);
}

// --- reserved 21-29

extern "x86-interrupt" fn security_exception_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "CPU EXCEPTION: SECURITY EXCEPTION: {}\n{:#?}",
        error_code, stack_frame
    );
}

// reserved 31
// --- triple fault (can't handle)

pub const EXCEPTION_IST_INDEX: u16 = 0;
pub const DOUBLE_FAULT_IST_INDEX: u16 = 1;
pub const ISR_IST_INDEX: u16 = 2;

// TODO this should all be in `kernel` and not `lib`
lazy_static::lazy_static! {
    static ref IDT: spin::Mutex<InterruptDescriptorTable> = {
        let mut idt = InterruptDescriptorTable::new();

        unsafe {
        // fault interrupts
        idt.divide_error.set_handler_fn(divide_error_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.debug.set_handler_fn(debug_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.non_maskable_interrupt
            .set_handler_fn(non_maskable_interrupt_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.breakpoint.set_handler_fn(breakpoint_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.overflow.set_handler_fn(overflow_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.bound_range_exceeded
            .set_handler_fn(bound_range_exceeded_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.invalid_opcode.set_handler_fn(invalid_opcode_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.device_not_available
            .set_handler_fn(device_not_available_handler).set_stack_index(EXCEPTION_IST_INDEX);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX)
        };
        idt.invalid_tss.set_handler_fn(invalid_tss_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.segment_not_present
            .set_handler_fn(segment_not_present_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.stack_segment_fault
            .set_handler_fn(stack_segment_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.page_fault.set_handler_fn(page_fault_handler).set_stack_index(EXCEPTION_IST_INDEX);
        // --- reserved 15
        idt.x87_floating_point
            .set_handler_fn(x87_floating_point_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.alignment_check.set_handler_fn(alignment_check_handler).set_stack_index(EXCEPTION_IST_INDEX);
        // --- machine check (platform specific, not required)
        idt.simd_floating_point
            .set_handler_fn(simd_floating_point_handler).set_stack_index(EXCEPTION_IST_INDEX);
        idt.virtualization.set_handler_fn(virtualization_handler).set_stack_index(EXCEPTION_IST_INDEX);
        // --- reserved 21-29
        idt.security_exception
            .set_handler_fn(security_exception_handler).set_stack_index(EXCEPTION_IST_INDEX);
        // --- triple fault (can't handle)
    }

        spin::Mutex::new(idt)
    };
}

pub unsafe fn load_unchecked() {
    let idt = IDT.lock();
    idt.load_unsafe()
}

pub fn set_handler_fn(vector: u8, handler: extern "x86-interrupt" fn(InterruptStackFrame)) {
    crate::instructions::interrupts::without_interrupts(|| {
        unsafe {
            IDT.lock()[vector as usize]
                .set_handler_fn(handler)
                .set_stack_index(ISR_IST_INDEX)
        };
    });
}
