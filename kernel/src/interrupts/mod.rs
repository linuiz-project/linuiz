mod exceptions;
mod stubs;

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

#[naked]
#[no_mangle]
extern "x86-interrupt" fn irq_common(_: InterruptStackFrame) {
    unsafe {
        core::arch::asm!(
        "
        # (QWORD) ISF should begin here on the stack. 
        # (QWORD) IRQ vector is here.
        # (QWORD) `call` return instruction pointer is here.

        # Push all gprs to the stack.
        push r15
        push r14
        push r13
        push r12
        push r11
        push r10
        push r9
        push r8
        push rbp
        push rdi
        push rsi
        push rdx
        push rcx
        push rbx
        push rax
    
        cld

        # Move IRQ vector into first parameter
        mov rcx, [rsp + (16 * 8)]
        # Move stack frame into second parameter.
        lea rdx, [rsp + (17 * 8)]
        # Move cached gprs pointer into third parameter.
        mov r8, rsp

        call {}
    
        pop rax
        pop rbx
        pop rcx
        pop rdx
        pop rsi
        pop rdi
        pop rbp
        pop r8
        pop r9
        pop r10
        pop r11
        pop r12
        pop r13
        pop r14
        pop r15

        # 'Pop' interrupt vector and return pointer
        add rsp, 0x10

        iretq
        ",
        sym irq_handoff,
        options(noreturn)
        );
    }
}

extern "win64" fn irq_handoff(
    irq_vector: u8,
    isf: &mut InterruptStackFrame,
    cached_regs: &mut crate::scheduling::ThreadRegisters,
) {
    if let Some(handler) = INTERRUPT_HANDLERS.read()[irq_vector as usize] {
        handler(isf, cached_regs);
    }
}

static mut IDT: spin::Mutex<InterruptDescriptorTable> = spin::Mutex::new(InterruptDescriptorTable::new());

pub type HandlerFunc = fn(&mut InterruptStackFrame, &mut crate::scheduling::ThreadRegisters);
static INTERRUPT_HANDLERS: spin::RwLock<[Option<HandlerFunc>; 256]> = spin::RwLock::new([None; 256]);

/// Initialize the global IDT's exception and stub handlers.
pub fn init_idt() {
    assert!(
        crate::tables::gdt::KCODE_SELECTOR.get().is_some(),
        "Cannot initialize IDT before GDT (IDT entries use GDT kernel code segment selector)."
    );

    let mut idt = unsafe { IDT.lock() };

    exceptions::set_exception_handlers(&mut *idt);
    stubs::set_stub_handlers(&mut *idt);
}

/// Loads the global IDT using `lidt`.
pub fn load_idt() {
    let idt = unsafe { IDT.lock() };
    unsafe { idt.load_unsafe() };
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Vector {
    GlobalTimer = 0x20,
    Error = 0x40,
    LocalTimer = 0x41,
    Performance = 0x46,
    ThermalSensor = 0x47,
    Storage0 = 0x50,
    Storage1 = 0x51,
    Storage2 = 0x52,

    Syscall = 0x80,

    /* CANNOT BE CHANGED — DEFAULT FROM APIC */
    LINT0_VECTOR = 253,
    LINT1_VECTOR = 254,
    SPURIOUS_VECTOR = 255,
}

/// Sets the interrupt handler function for the given vector.
///
/// SAFETY: This function is unsafe because any (including a malformed or buggy) handler can be
///         specified. The caller of this function must ensure the handler is correctly formed,
///         and properly handles the interrupt it is being assigned to.  
pub unsafe fn set_handler_fn(vector: Vector, handler: HandlerFunc) {
    assert!(
        crate::tables::gdt::KCODE_SELECTOR.get().is_some(),
        "Cannot assign IDT handlers before GDT init (IDT entries use GDT kernel code segment selector)."
    );

    libarch::instructions::interrupts::without_interrupts(|| {
        INTERRUPT_HANDLERS.write()[vector as usize] = Some(handler);
    });
}

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
pub enum StackTableIndex {
    Debug = 0,
    NonMaskable = 1,
    DoubleFault = 2,
    MachineCheck = 3,
}

pub fn syscall_interrupt_handler(
    _: &mut x86_64::structures::idt::InterruptStackFrame,
    gprs: &mut crate::scheduling::ThreadRegisters,
) {
    let control_ptr = gprs.rdi as *mut libkernel::syscall::Control;

    if !crate::memory::get_kernel_page_manager()
        .unwrap()
        .is_mapped(libkernel::Address::<libkernel::Virtual>::from_ptr(control_ptr))
    {
        gprs.rsi = libkernel::syscall::Error::ControlNotMapped as u64;
        return;
    }

    gprs.rsi = 0xD3ADC0DA;
}
