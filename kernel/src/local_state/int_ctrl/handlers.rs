use crate::{scheduling::ThreadRegisters, tables::idt::InterruptStackFrame};

#[naked]
#[allow(named_asm_labels)]
pub extern "x86-interrupt" fn apit_handler(_: InterruptStackFrame) {
    unsafe {
        core::arch::asm!(
            "
            /* Push all gprs to the stack. */
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

            /* Move stack frame into first parameter. */
            lea rcx, [rsp + (15 * 8)]
            /* Move cached gprs pointer into second parameter. */
            mov rdx, rsp

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


            iretq
            ",
            sym apit_handler_inner,
            options(noreturn)
        )
    };
}

pub extern "x86-interrupt" fn storage_handler(_: InterruptStackFrame) {
    crate::local_state::int_ctrl().end_of_interrupt();
}

extern "win64" fn apit_handler_inner(
    stack_frame: &mut InterruptStackFrame,
    cached_regs: *mut ThreadRegisters,
) {
    const THREAD_LOCK_FAIL_PERIOD_MS: u32 = 1;
    const DEFAULT_SCHEDULING_PERIOD_MS: u32 = 20;

    let time_slice_ms = crate::local_state::try_lock_scheduler().map_or(
        THREAD_LOCK_FAIL_PERIOD_MS,
        |mut thread| match thread.run_next(stack_frame, cached_regs) {
            0 => DEFAULT_SCHEDULING_PERIOD_MS,
            period_ms => period_ms as u32,
        },
    );

    let int_ctrl = crate::local_state::int_ctrl();
    int_ctrl.reload_timer(core::num::NonZeroU32::new(time_slice_ms));
    int_ctrl.end_of_interrupt();
}

pub extern "x86-interrupt" fn error_handler(_: InterruptStackFrame) {
    let apic = &crate::local_state::int_ctrl().apic;
    error!("APIC ERROR INTERRUPT");
    error!("--------------------");
    error!("DUMPING APIC ERROR REGISTER:");
    error!("  {:?}", apic.error_status());

    apic.end_of_interrupt();
}

pub extern "x86-interrupt" fn spurious_handler(_: InterruptStackFrame) {
    // Perhaps don't need to EOI spurious interrupts?
    // crate::lpu::try_get().unwrap().int_ctrl().end_of_interrupt();
}
