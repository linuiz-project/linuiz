use crate::{
    arch::x86_64::structures::idt::InterruptStackFrame,
    cpu::local_state::LocalState,
    mem::stack::Stack,
    task::{Registers, Task},
};
use alloc::{boxed::Box, collections::vec_deque::VecDeque};
use core::{alloc::AllocError, time::Duration};
use libsys::Address;
use zerocopy::FromZeros;

pub static PROCESSES: spin::Mutex<VecDeque<Task>> = spin::Mutex::new(VecDeque::new());

pub struct Scheduler {
    enabled: bool,
    idle_stack: Box<Stack<0x1000>>,
    task: Option<Task>,
}

impl Scheduler {
    pub fn new() -> Result<Self, AllocError> {
        Ok(Self {
            enabled: false,
            idle_stack: Stack::new_box_zeroed().map_err(|_| AllocError)?,
            task: None,
        })
    }

    /// Enables the scheduler to pop tasks.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disables scheduler from popping tasks. Any task pops which are already in-flight will not be cancelled.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Indicates whether the scheduler is enabled.
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub const fn process(&self) -> Option<&Task> {
        self.task.as_ref()
    }

    pub fn task_mut(&mut self) -> Option<&mut Task> {
        self.task.as_mut()
    }

    pub fn interrupt_task(&mut self, state: &mut InterruptStackFrame, regs: &mut Registers) {
        debug_assert!(!crate::interrupts::is_enabled());

        let mut processes = PROCESSES.lock();

        // Move the current task, if any, back into the scheduler queue.
        if let Some(mut process) = self.task.take() {
            trace!("Interrupting: {:?}", process.id());

            process.context.0 = *state;
            process.context.1 = *regs;

            processes.push_back(process);
        }

        self.next_task(&mut processes, state, regs);
    }

    /// Attempts to schedule the next task in the local task queue.
    pub fn yield_task(&mut self, isf: &mut InterruptStackFrame, regs: &mut Registers) {
        debug_assert!(!crate::interrupts::is_enabled());

        let mut processes = PROCESSES.lock();

        let mut process = self.task.take().expect("no active task in scheduler");
        trace!("Yielding: {:?}", process.id());

        process.context.0 = *isf;
        process.context.1 = *regs;

        processes.push_back(process);

        self.next_task(&mut processes, isf, regs);
    }

    pub fn kill_task(&mut self, isf: &mut InterruptStackFrame, regs: &mut Registers) {
        debug_assert!(!crate::interrupts::is_enabled());

        // TODO add process to reap queue to reclaim address space memory
        let process = self.task.take().expect("no active task in scheduler");
        trace!("Exiting: {:?}", process.id());

        let mut processes = PROCESSES.lock();
        self.next_task(&mut processes, isf, regs);
    }

    fn next_task(
        &mut self,
        processes: &mut VecDeque<Task>,
        isf: &mut InterruptStackFrame,
        regs: &mut Registers,
    ) {
        // Pop a new task from the task queue, or simply switch in the idle task.
        if let Some(next_process) = processes.pop_front() {
            *isf = next_process.context.0;
            *regs = next_process.context.1;

            if !next_process.address_space.is_current() {
                // Safety: New task requires its own address space.
                unsafe {
                    next_process.address_space.swap_into();
                }
            }

            trace!("Switched task: {:?}", next_process.id());
            let old_value = self.task.replace(next_process);
            debug_assert!(old_value.is_none());
        } else {
            // Safety: Instruction pointer is to a valid function.
            #[allow(clippy::as_conversions)]
            unsafe {
                isf.set_instruction_pointer(
                    Address::new(crate::interrupts::wait_indefinite as usize).unwrap(),
                );
            }

            // Safety: Stack pointer is valid for idle function stack.
            unsafe {
                isf.set_stack_pointer(Address::new(self.idle_stack.top().addr().get()).unwrap());
            }

            *regs = Registers::empty();

            trace!("Switched idle task.");
        }

        // TODO have some kind of queue of preemption waits, to ensure we select the shortest one.
        // Safety: Just having switched tasks, no preemption wait should supercede this one.
        unsafe {
            LocalState::set_preemption_wait(Duration::from_millis(15));
        }
    }
}

// #[cfg(target_arch = "x86_64")]
// #[naked]
// unsafe extern "sysv64" fn exit_into(regs: &mut Registers, state: &mut State) -> ! {
//     use core::mem::size_of;
//     use x86_64::structures::idt::InterruptStackFrame;

//     core::arch::asm!(
//         "
//         mov rax, rdi    # registers ptr

//         sub rsp, {0}    # make space for stack frame
//         # state ptr is already in `rsi` from args
//         mov rdi, rsp    # dest is stack address
//         mov rcx, {0}    # set the copy length

//         cld             # clear direction for op
//         rep movsb       # copy memory

//         mov rbx, [rax + (1 * 8)]
//         mov rcx, [rax + (2 * 8)]
//         mov rdx, [rax + (3 * 8)]
//         mov rsi, [rax + (4 * 8)]
//         mov rdi, [rax + (5 * 8)]
//         mov rbp, [rax + (6 * 8)]
//         mov r8, [rax + (7 * 8)]
//         mov r9, [rax + (8 * 8)]
//         mov r10, [rax + (9 * 8)]
//         mov r11, [rax + (10 * 8)]
//         mov r12, [rax + (11 * 8)]
//         mov r13, [rax + (12 * 8)]
//         mov r14, [rax + (13 * 8)]
//         mov r15, [rax + (14 * 8)]
//         mov rax, [rax + (0 * 8)]

//         iretq
//         ",
//         const size_of::<InterruptStackFrame>(),
//         options(noreturn)
//     )
// }
