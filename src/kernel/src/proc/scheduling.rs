use crate::proc::task::Task;
use alloc::collections::BinaryHeap;

pub struct Scheduler {
    enabled: bool,
    total_priority: u64,
    idle_task: Task,
    cur_task: Option<Task>,
    tasks: BinaryHeap<Task>,
}

impl Scheduler {
    pub fn new(enabled: bool, idle_task: Task) -> Self {
        Self { enabled, total_priority: 0, idle_task, cur_task: None, tasks: BinaryHeap::new() }
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

    /// Pushes a new task to the scheduling queue.
    pub fn push_task(&mut self, task: Task) {
        self.total_priority += task.priority() as u64;
        self.tasks.push(task);
    }

    /// If the scheduler is enabled, attempts to return a new task from
    /// the task queue. Returns `None` if the queue is empty.
    pub fn pop_task(&mut self) -> Option<Task> {
        if self.enabled {
            self.tasks.pop().map(|task| {
                self.total_priority -= task.priority() as u64;
                task
            })
        } else {
            None
        }
    }

    #[inline]
    pub const fn get_total_priority(&self) -> u64 {
        self.total_priority
    }

    #[inline]
    pub const fn current_task(&self) -> Option<&Task> {
        self.cur_task.as_ref()
    }

    /// Attempts to schedule the next task in the local task queue.
    pub fn next_task(
        &mut self,
        state: &mut super::State,
        regsters: &mut super::Registers,
    ) {
        debug_assert!(!crate::interrupts::are_enabled());

        // Move the current task, if any, back into the scheduler queue.
        if let Some(mut cur_task) = self.cur_task.take() {
            cur_task.ctrl_flow_context = *s;
            cur_task.arch_context = *arch_context;

            trace!("Reclaiming task: {:?}", cur_task.uuid());
            self.push_task(cur_task);
        } else {
            self.idle_task.ctrl_flow_context = *ctrl_flow_context;
            self.idle_task.arch_context = *arch_context;

            trace!("Reclaiming idle task.");
        }

        // Pop a new task from the task queue, or simply switch in the idle task.
        if let Some(next_task) = self.pop_task() {
            trace!("Switching task: {:?}", next_task.uuid());

            *ctrl_flow_context = next_task.ctrl_flow_context;
            *arch_context = next_task.arch_context;
            next_task.with_address_space(|address_space| {
                // Safety: New task requires its own address space.
                unsafe { address_space.swap_into() }
            });

            trace!("Switched task: {:?}", next_task.uuid());

            self.cur_task = Some(next_task);
        } else {
            trace!("Switching idle task.");

            let default_task = &self.idle_task;
            *ctrl_flow_context = default_task.ctrl_flow_context;
            *arch_context = default_task.arch_context;
            default_task.with_address_space(|address_space| {
                // Safety: New task requires its own address space.
                unsafe { address_space.swap_into() }
            });

            trace!("Switched idle task.");
        };

        // TODO have some kind of queue of preemption waits, to ensure we select the shortest one.
        // Safety: Just having switched tasks, no preemption wait should supercede this one.
        unsafe {
            const TIME_SLICE: core::num::NonZeroU16 = core::num::NonZeroU16::new(5).unwrap();

            crate::local_state::set_preemption_wait(TIME_SLICE);
        }
    }
}
