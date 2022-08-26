use core::arch::asm;
use core::cell::SyncUnsafeCell;
use num_enum::TryFromPrimitive;

/// Enables interrupts for the current core.
///
/// SAFETY: Enabling interrupts early can result in unexpected behaviour.
#[inline(always)]
pub unsafe fn enable() {
    #[cfg(target_arch = "x86_64")]
    asm!("sti", options(nostack, nomem));

    #[cfg(target_arch = "riscv64")]
    crate::arch::rv64::registers::sstatus::set_sie(true);
}

/// Disables interrupts for the current core.
///
/// SAFETY: Disabling interrupts can cause the system to become unresponsive if they are not re-enabled.
#[inline(always)]
pub unsafe fn disable() {
    #[cfg(target_arch = "x86_64")]
    asm!("cli", options(nostack, nomem));

    #[cfg(target_arch = "riscv64")]
    crate::arch::rv64::registers::sstatus::set_sie(false);
}

/// Returns whether or not interrupts are enabled for the current core.
#[inline(always)]
pub fn are_enabled() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x64::registers::RFlags::read().contains(crate::arch::x64::registers::RFlags::INTERRUPT_FLAG)
    }

    #[cfg(target_arch = "riscv64")]
    {
        crate::arch::rv64::registers::sstatus::get_sie()
    }
}

/// Disables interrupts, executes the given [`FnOnce`], and re-enables interrupts if they were prior.
pub fn without<R>(func: impl FnOnce() -> R) -> R {
    let interrupts_enabled = are_enabled();

    if interrupts_enabled {
        unsafe { disable() };
    }

    let return_value = func();

    if interrupts_enabled {
        unsafe { enable() };
    }

    return_value
}

/// Waits for the next interrupt on the current core.
#[inline(always)]
pub fn wait_for() {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        asm!("hlt", options(nostack, nomem, preserves_flags));

        #[cfg(target_arch = "riscv64")]
        asm!("wfi", options(nostack, nomem, preserves_flags));
    }
}

/// Indefinitely waits for the next interrupt on the current core.
#[inline(always)]
pub fn wait_loop() -> ! {
    loop {
        wait_for()
    }
}

const PIC_BASE: u8 = 0xE0;

/// Delivery mode for IPIs.
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

    Syscall = 0x30,
    Timer = 0x31,
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

#[cfg(target_arch = "x86_64")]
pub type ArchContext = (crate::arch::x64::cpu::GeneralContext, crate::arch::x64::cpu::SpecialContext);

pub fn common_interrupt_handler(
    irq_vector: u64,
    ctrl_flow_context: &mut ControlFlowContext,
    arch_context: &mut ArchContext,
) {
    match Vector::try_from(irq_vector) {
        Ok(vector) if vector == Vector::Timer => {
            crate::local_state::schedule_next_task(ctrl_flow_context, arch_context);
        }

        Ok(vector) if vector == Vector::Syscall => {
            // TODO general syscall impl
            #[cfg(target_arch = "x86_64")]
            {
                let control_ptr = arch_context.0.rdi as *mut libkernel::syscall::Control;

                if !crate::memory::get_kernel_page_manager()
                    .is_mapped(libkernel::memory::Page::from_index((control_ptr as usize) / 0x1000))
                {
                    arch_context.0.rsi = libkernel::syscall::Error::ControlNotMapped as u64;
                    return;
                }

                arch_context.0.rsi = 0xDEADC0DE;
            }
        }

        vector_result => {
            warn!("Unhandled IRQ vector: {:?}", vector_result);
        }
    }

    #[cfg(target_arch = "x86_64")]
    crate::arch::x64::structures::apic::end_of_interrupt();
}

#[cfg(target_arch = "x86_64")]
pub type ArchException = crate::arch::x64::structures::idt::Exception;

/* EXCEPTION HANDLING */
type ExceptionHandler = fn(ArchException);
static EXCEPTION_HANDLER: SyncUnsafeCell<ExceptionHandler> =
    SyncUnsafeCell::new(|exception| panic!("\n{:#?}", exception));

/// Sets the common exception handler.
///
/// SAFETY: The caller must ensure the provided function handles exceptions in a valid way.
pub unsafe fn set_common_exception_handler(handler: ExceptionHandler) {
    *EXCEPTION_HANDLER.get() = handler;
}

/// Gets the current common exception handler.
#[inline]
pub fn get_common_exception_handler() -> &'static ExceptionHandler {
    unsafe { &*EXCEPTION_HANDLER.get() }
}

/* NON-EXCEPTION IRQ HANDLING */
type InterruptHandler = fn(u64, &mut ControlFlowContext, &mut ArchContext);
static INTERRUPT_HANDLER: SyncUnsafeCell<InterruptHandler> = SyncUnsafeCell::new(common_interrupt_handler);

/// Sets the common interrupt handler.
///
/// SAFETY: The caller must ensure the provided function handles interrupts in a valid way.
pub unsafe fn set_common_interrupt_handler(handler: InterruptHandler) {
    *INTERRUPT_HANDLER.get() = handler;
}

/// Gets the current common interrupt handler.
#[inline]
pub fn get_common_interrupt_handler() -> &'static InterruptHandler {
    unsafe { &*INTERRUPT_HANDLER.get() }
}
