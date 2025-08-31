use crate::{
    arch::x86_64::structures::idt::InterruptStackFrame,
    cpu::local_state::LocalState,
    task::{Registers, Task},
    util::sync::Mutex,
};
use core::{mem::MaybeUninit, num::NonZero, ptr::NonNull, time::Duration};

type TaskQueue<'a> = heapless::Deque<Task<'a>, 100>;

pub static PROCESSES: Mutex<TaskQueue> = Mutex::new(TaskQueue::new());

#[cfg(debug_assertions)]
const IDLE_STACK_SIZE: usize = 0x100;
#[cfg(not(debug_assertions))]
const IDLE_STACK_SIZE: usize = 0x20;

#[repr(align(0x10))]
struct IdleStack([MaybeUninit<u8>; IDLE_STACK_SIZE]);

impl IdleStack {
    fn top(&self) -> NonNull<u8> {
        // Safety: `self.0` max index is `self.0.len() - 1`.
        let top_byte = unsafe { self.0.get_unchecked(self.0.len() - 1) };
        let top_ptr = core::ptr::from_ref(top_byte).cast_mut().cast::<u8>();

        // Safety: `top_ptr` is derived from `self.0`, and so cannot be null.
        unsafe { NonNull::new_unchecked(top_ptr) }
    }
}

pub struct Scheduler {
    enabled: bool,
    idle_stack: IdleStack,
    task: Option<Task<'static>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            enabled: false,
            idle_stack: IdleStack(core::array::repeat(MaybeUninit::uninit())),
            task: None,
        }
    }

    /// Enables the scheduler to pop tasks.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disables scheduler from popping tasks. Any task pops which are already
    /// in-flight will not be cancelled.
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

    pub fn task_mut(&mut self) -> Option<&'static mut Task> {
        self.task.as_mut()
    }

    pub fn interrupt_task(&mut self, state: &mut InterruptStackFrame, regs: &mut Registers) {
        PROCESSES.with_lock(|processes| {
            // Move the current task, if any, back into the scheduler queue.
            if let Some(mut process) = self.task.take() {
                trace!("Interrupting: {:?}", process.id());

                process.context.0 = *state;
                process.context.1 = *regs;

                processes.push_back(process).unwrap();
            }

            self.next_task(processes, state, regs);
        });
    }

    /// Attempts to schedule the next task in the local task queue.
    pub fn yield_task(&mut self, isf: &mut InterruptStackFrame, regs: &mut Registers) {
        PROCESSES.with_lock(|processes| {
            let mut process = self.task.take().expect("no active task in scheduler");
            trace!("Yielding: {:?}", process.id());

            process.context.0 = *isf;
            process.context.1 = *regs;

            processes.push_back(process).unwrap();

            self.next_task(processes, isf, regs);
        });
    }

    pub fn kill_task(&mut self, isf: &mut InterruptStackFrame, regs: &mut Registers) {
        debug_assert!(!crate::interrupts::is_enabled());

        // TODO add process to reap queue to reclaim address space memory
        let process = self.task.take().expect("no active task in scheduler");
        trace!("Exiting: {:?}", process.id());

        PROCESSES.with_lock(|processes| {
            self.next_task(processes, isf, regs);
        });
    }

    fn next_task(
        &mut self,
        processes: &mut TaskQueue,
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

            todo!()
            // let old_value = self.task.replace(next_process);
            // debug_assert!(old_value.is_none());
        } else {
            #[allow(clippy::as_conversions)]
            let idle_wait_address = crate::interrupts::wait_indefinite as usize;
            // Safety: Function address cannot be zero.
            let idle_wait_address = unsafe { NonZero::<usize>::new_unchecked(idle_wait_address) };
            let idle_wait_ptr = NonNull::<u8>::with_exposed_provenance(idle_wait_address);

            // Safety: Instruction pointer is to a valid function.
            unsafe {
                isf.set_instruction_pointer(Some(idle_wait_ptr));
            }

            // Safety: Stack pointer is valid for idle function stack.
            unsafe {
                isf.set_stack_pointer(Some(self.idle_stack.top()));
            }

            *regs = Registers::empty();

            trace!("Switched idle task.");
        }

        // TODO have some kind of queue of preemption waits, to ensure we select the
        // shortest one.
        // Safety: No preemption wait will supercede this one.
        unsafe {
            LocalState::set_preemption_wait(Duration::from_millis(15));
        }
    }
}

// #[cfg(target_arch = "x86_64")]
// #[naked]
// unsafe extern "sysv64" fn exit_into(regs: &mut Registers, state: &mut State)
// -> ! {     use core::mem::size_of;
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
