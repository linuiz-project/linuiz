use crate::{
    cpu::context::Context,
    mem::{HigherHalfDirectMap, pmm::PhysicalMemoryManager},
};
use core::{mem::MaybeUninit, num::NonZero, ptr::NonNull};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use uuid::Uuid;

mod task;
use task::Task;

type TaskQueue = heapless::mpmc::Queue<Task, 1024>;

pub static TASKS: TaskQueue = TaskQueue::new();

#[repr(usize)]
#[derive(Debug, Error, IntoPrimitive, TryFromPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum YieldTaskError {
    #[error("there was no active task on this processor")]
    NoActiveTask = 1,
}

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
    task: Option<Task>,
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

    fn next_task(&mut self, context: &mut Context) {
        if let Some(next_task) = TASKS.dequeue() {
            *context = next_task.context().clone();

            // TODO Manage the address space switch.

            trace!("Switched: {:?}", next_task.id());

            self.task = Some(next_task);
        } else {
            *context = Context::new_idle(self.idle_stack.top().addr());

            trace!("Switched: IDLE");
        }
    }

    pub fn interrupt_task(&mut self, context: &mut Context) {
        // Move the current task, if any, back into the scheduler queue.
        if let Some(mut task) = self.task.take() {
            trace!("Interrupting: {:?}", task.id());

            *task.context_mut() = context.clone();

            TASKS.enqueue(task).unwrap();
        }

        self.next_task(context);
    }

    /// Attempts to schedule the next task in the local task queue.
    pub fn yield_task(&mut self, context: &mut Context) -> Result<(), YieldTaskError> {
        let mut task = self.task.take().ok_or(YieldTaskError::NoActiveTask)?;
        trace!("Yielding: {:?}", task.id());

        *task.context_mut() = context.clone();
        self.next_task(context);

        Ok(())
    }

    // pub fn kill_task(&mut self, isf: &mut InterruptStackFrame, regs: &mut
    // Registers) {     debug_assert!(!crate::interrupts::is_enabled());

    //     // TODO add process to reap queue to reclaim address space memory
    //     let process = self.task.take().expect("no active task in scheduler");
    //     trace!("Exiting: {:?}", process.id());

    //     PROCESSES.with_lock(|processes| {
    //         self.next_task(processes, isf, regs);
    //     });
    // }

    //     // TODO have some kind of queue of preemption waits, to ensure we select
    // the     // shortest one.
    //     // Safety: No preemption wait will supercede this one.
    //     unsafe {
    //         LocalState::set_preemption_wait(Duration::from_millis(15));
    //     }
    // }

    fn kill_current_task(&mut self) {
        todo!()
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

pub fn queue_procedure(procedure_fn: fn()) {
    /// This function is technically unsafe, but the signature must be
    /// maintained for use with [`Context::new_from_fn_with_arg`].
    ///
    /// # Safety
    ///
    /// - `address` must be the address to an `extern "Rust" fn()`.
    extern "sysv64" fn dispatch_procedure(address: usize) {
        let procedure_ptr = core::ptr::with_exposed_provenance::<()>(address);
        // Safety: Caller is required to maintain safety invariants.
        let procedure_fn = unsafe { core::mem::transmute::<*const (), fn()>(procedure_ptr) };

        procedure_fn();

        crate::cpu::local_state::LocalState::with_scheduler(Scheduler::kill_current_task);
    }

    let stack_address = PhysicalMemoryManager::next_free(NonZero::<usize>::MIN, false)
        .unwrap()
        .get()
        .get();

    #[allow(clippy::as_conversions)]
    let context = Context::new_from_fn_with_arg(
        dispatch_procedure,
        procedure_fn as usize,
        HigherHalfDirectMap::offset(stack_address),
        false,
    );

    let procedure = Task::Procedure {
        id: Uuid::new_v4(),
        context,
    };

    trace!("Queueing procedure: {procedure:?}");

    TASKS.enqueue(procedure).unwrap();
}
