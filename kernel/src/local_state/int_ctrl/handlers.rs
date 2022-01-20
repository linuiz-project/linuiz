use core::{
    ops::Add,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::scheduling::{Task, TaskRegisters};
use libstd::structures::idt::InterruptStackFrame;

#[naked]
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

            /* Move stack frame into first parameter. */
            mov rcx, rsp
            add rcx, 15 * 8 /* ISF will be just before the 14 registers we pushed. */
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
            sym trap_handler,
            options(noreturn)
        )
    };
}

pub extern "x86-interrupt" fn storage_handler(_: InterruptStackFrame) {
    crate::local_state::int_ctrl().unwrap().end_of_interrupt();
}

pub static mut TASK_1: Option<Task> = None;
pub static mut TASK_2: Option<Task> = None;

static IS_TASK_1: AtomicBool = AtomicBool::new(false);
static IS_FINISHED: AtomicBool = AtomicBool::new(false);
fn task1() {
    loop {
        while IS_FINISHED.load(Ordering::Acquire) {}

        crate::println!(".");
        IS_FINISHED.store(true, Ordering::Release)
    }
}

fn task2() {
    loop {
        while IS_FINISHED.load(Ordering::Acquire) {}

        crate::println!("!");
        IS_FINISHED.store(true, Ordering::Release)
    }
}

extern "win64" fn trap_handler(
    stack_frame: &mut InterruptStackFrame,
    cached_regs: &mut TaskRegisters,
) {
    let stack_frame_value = unsafe { stack_frame.as_mut().read() };

    let task = if IS_TASK_1.load(Ordering::Relaxed) {
        crate::println!("Switching to Task 2.");
        IS_TASK_1.store(false, Ordering::Relaxed);

        unsafe {
            TASK_2.get_or_insert_with(|| {
                let stack = libstd::memory::malloc::try_get()
                    .unwrap()
                    .alloc(0x1000, None)
                    .unwrap()
                    .into_slice();

                Task {
                    rip: task2 as u64,
                    cs: stack_frame_value.code_segment,
                    rsp: stack.as_ptr().add(stack.len()) as u64,
                    ss: stack_frame_value.stack_segment,
                    rfl: libstd::registers::RFlags::minimal().bits(),
                    gprs: TaskRegisters::empty(),
                    stack,
                }
            })
        }
    } else {
        crate::println!("Switching to Task 1.");
        IS_TASK_1.store(true, Ordering::Relaxed);

        unsafe {
            TASK_1.get_or_insert_with(|| {
                let stack = libstd::memory::malloc::try_get()
                    .unwrap()
                    .alloc(0x1000, None)
                    .unwrap()
                    .into_slice();

                Task {
                    rip: task1 as u64,
                    cs: stack_frame_value.code_segment,
                    rsp: stack.as_ptr().add(stack.len()) as u64,
                    ss: stack_frame_value.stack_segment,
                    rfl: libstd::registers::RFlags::minimal().bits(),
                    gprs: TaskRegisters::empty(),
                    stack,
                }
            })
        }
    };

    use x86_64::VirtAddr;

    unsafe {
        stack_frame
            .as_mut()
            .write(x86_64::structures::idt::InterruptStackFrameValue {
                instruction_pointer: VirtAddr::new_truncate(task.rip),
                code_segment: task.cs,
                cpu_flags: task.rfl,
                stack_pointer: VirtAddr::new_truncate(task.rsp),
                stack_segment: task.ss,
            })
    };

    *cached_regs = task.gprs;

    crate::local_state::clock().unwrap().tick();
    crate::local_state::int_ctrl().unwrap().end_of_interrupt();
    IS_FINISHED.store(false, Ordering::Release);
}

pub extern "x86-interrupt" fn error_handler(_: InterruptStackFrame) {
    let apic = &crate::local_state::int_ctrl().unwrap().apic;
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
