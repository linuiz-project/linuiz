use crate::{
    memory::Stack,
    proc::{Process, Registers, State},
};
use alloc::collections::VecDeque;

pub static PROCESSES: spin::Mutex<VecDeque<Process>> = spin::Mutex::new(VecDeque::new());

pub struct Scheduler {
    enabled: bool,
    process: Option<Process>,
}

impl Scheduler {
    pub fn new(enabled: bool) -> Self {
        Self { enabled, process: None }
    }

    /// Enables the scheduler to pop tasks.
    #[inline]
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disables scheduler from popping tasks. Any task pops which are already in-flight will not be cancelled.
    #[inline]
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Indicates whether the scheduler is enabled.
    #[inline]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[inline]
    pub const fn process(&self) -> Option<&Process> {
        self.process.as_ref()
    }

    #[inline]
    pub fn process_mut(&mut self) -> Option<&mut Process> {
        self.process.as_mut()
    }

    // /// Pushes a new task to the scheduling queue.
    // pub fn push_task(&mut self, task: Task) {
    //     self.total_priority += task.priority() as u64;
    //     self.tasks.push(task);
    // }

    // /// If the scheduler is enabled, attempts to return a new task from
    // /// the task queue. Returns `None` if the queue is empty.
    // pub fn pop_task(&mut self) -> Option<Task> {
    //     if self.enabled {
    //         self.tasks.pop().map(|task| {
    //             self.total_priority -= task.priority() as u64;
    //             task
    //         })
    //     } else {
    //         None
    //     }
    // }

    // #[inline]
    // pub const fn get_total_priority(&self) -> u64 {
    //     self.total_priority
    // }

    // #[inline]
    // pub const fn current_task(&self) -> Option<&Task> {
    //     self.cur_task.as_ref()
    // }

    /// Attempts to schedule the next task in the local task queue.
    pub fn next_task(&mut self, state: &mut super::State, regs: &mut super::Registers) {
        debug_assert!(!crate::interrupts::are_enabled());

        let mut processes = PROCESSES.lock();

        // Move the current task, if any, back into the scheduler queue.
        if let Some(mut process) = self.process.take() {
            process.context.0 = *state;
            process.context.1 = *regs;

            trace!("Reclaiming task: {:?}", process.uuid());
            processes.push_back(process);
        }

        // Pop a new task from the task queue, or simply switch in the idle task.
        if let Some(next_process) = processes.pop_front() {
            trace!("Switching task: {:?}", next_process.uuid());

            *state = next_process.context.0;
            *regs = next_process.context.1;

            next_process.with_address_space(|address_space| {
                // Safety: New task requires its own address space.
                unsafe { address_space.swap_into() }
            });

            trace!("Switched task: {:?}", next_process.uuid());

            let old_value = self.process.replace(next_process);
            assert!(old_value.is_none());
        } else {
            // This is linked to .bss directly rather than marked as a `static mut` to avoid actualy mutable usage.
            #[link_section = ".bss"]
            static IDLE_STACK: Stack<0x100> = Stack::new();

            trace!("Switching idle task.");

            *state = State { ip: crate::interrupts::wait_loop as u64, sp: IDLE_STACK.top().get() as u64 };
            *regs = Registers::user_default();

            trace!("Switched idle task.");
        };

        // TODO have some kind of queue of preemption waits, to ensure we select the shortest one.
        // Safety: Just having switched tasks, no preemption wait should supercede this one.
        unsafe {
            const TIME_SLICE: core::num::NonZeroU16 = core::num::NonZeroU16::new(5).unwrap();

            crate::local::set_preemption_wait(TIME_SLICE);
        }
    }
}
