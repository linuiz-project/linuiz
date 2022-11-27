use crate::proc::task::Task;
use alloc::collections::BinaryHeap;
use core::sync::atomic::AtomicU64;

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);

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

    /// Disables scheduler from popping tasks.
    ///
    /// REMARK: Any task pops which are already in-flight will not be cancelled.
    #[inline]
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Indicates whether the scheduler is enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
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

    pub fn get_total_priority(&self) -> u64 {
        self.total_priority
    }

    /// Attempts to schedule the next task in the local task queue.
    pub fn next_task(
        &mut self,
        ctrl_flow_context: &mut crate::cpu::ControlContext,
        arch_context: &mut crate::cpu::ArchContext,
    ) {
        use crate::memory::PagingRegister;

        const TIME_SLICE: u16 = 5;

        debug_assert!(!crate::interrupts::are_enabled());

        // Move the current task, if any, back into the scheduler queue.
        if let Some(mut cur_task) = self.cur_task.take() {
            cur_task.ctrl_flow_context = *ctrl_flow_context;
            cur_task.arch_context = *arch_context;
            cur_task.root_page_table_args = PagingRegister::read();

            self.push_task(cur_task);
        }

        // {
        //     let mut waiting_tasks = WAITING_TASKS.lock();
        //     if waiting_tasks.len() > 0 && let Some(new_task) = waiting_tasks.pop_front() {
        //         self.push_task(new_task);
        //     }
        // }

        unsafe {
            if let Some(next_task) = self.pop_task() {
                // Modify interrupt contexts (usually, the registers).
                *ctrl_flow_context = next_task.ctrl_flow_context;
                *arch_context = next_task.arch_context;

                // Set current page tables.
                PagingRegister::write(&next_task.root_page_table_args);

                self.cur_task = Some(next_task);
            } else {
                let default_task = &self.idle_task;

                // Modify interrupt contexts (usually, the registers).
                *ctrl_flow_context = default_task.ctrl_flow_context;
                *arch_context = default_task.arch_context;

                // Set current page tables.
                PagingRegister::write(&default_task.root_page_table_args);
            };

            crate::local_state::preemption_wait(core::num::NonZeroU16::new_unchecked(TIME_SLICE));
        }
    }
}
