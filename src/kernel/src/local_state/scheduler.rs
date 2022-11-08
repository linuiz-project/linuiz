use crate::memory::Stack;
use core::sync::atomic::AtomicU64;
use lzalloc::deque::Deque;

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);

pub enum EntryPoint {
    Address(libcommon::Address<libcommon::Virtual>),
    Function(fn() -> !),
}

static WAITING_TASKS: spin::Mutex<Deque<Task>> = spin::Mutex::new(Deque::new());

pub fn queue_task(new_task: Task) {
    crate::interrupts::without(|| {
        let mut waiting_tasks = WAITING_TASKS.lock();
        waiting_tasks.push_back(new_task).unwrap();
    })
}

// TODO move `Task` and its types / impls to a module
/// Representation object for different contexts of execution in the CPU.
pub struct Task {
    id: u64,
    prio: u8,
    stack: Stack,
    //pcid: Option<PCID>,
    pub ctrl_flow_context: crate::cpu::ControlContext,
    pub arch_context: crate::cpu::ArchContext,
    pub root_page_table_args: crate::memory::VmemRegister,
}

impl Task {
    pub fn new(
        priority: u8,
        start: EntryPoint,
        stack: Stack,
        arch_context: crate::cpu::ArchContext,
        root_page_table_args: crate::memory::VmemRegister,
    ) -> Self {
        Self {
            id: NEXT_THREAD_ID.fetch_add(1, core::sync::atomic::Ordering::AcqRel),
            prio: priority,
            stack,
            ctrl_flow_context: crate::cpu::ControlContext {
                ip: match start {
                    EntryPoint::Address(address) => address.as_u64(),
                    EntryPoint::Function(function) => function as usize as u64,
                },
                // ### Safety:
                sp: stack.last().map(|v| v as *const u8).unwrap_or(core::ptr::null()),
            },
            arch_context,
            root_page_table_args,
        }
    }

    /// Returns this task's ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the [`TaskPriority`] struct for this task.
    pub fn priority(&self) -> u8 {
        self.prio
    }
}

impl core::fmt::Debug for Task {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_struct("Task").field("Priority", &self.prio).finish()
    }
}

pub struct Scheduler {
    enabled: bool,
    total_priority: u64,
    idle_task: Task,
    cur_task: Option<Task>,
    tasks: Deque<Task>,
}

impl Scheduler {
    pub fn new(enabled: bool, idle_task: Task) -> Self {
        Self { enabled, total_priority: 0, idle_task, cur_task: None, tasks: Deque::new() }
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
        self.total_priority += task.priority().get() as u64;
        self.tasks.push_back(task).unwrap();
    }

    /// If the scheduler is enabled, attempts to return a new task from
    /// the task queue. Returns `None` if the queue is empty.
    pub fn pop_task(&mut self) -> Option<Task> {
        if self.enabled {
            self.tasks.pop_front().map(|task| {
                self.total_priority -= task.priority().get() as u64;
                task
            })
        } else {
            None
        }
    }

    pub fn get_avg_prio(&self) -> u64 {
        self.total_priority.checked_div(self.tasks.len() as u64).unwrap_or(0)
    }

    #[inline]
    pub fn get_task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Attempts to schedule the next task in the local task queue.
    pub fn next_task(
        &mut self,
        ctrl_flow_context: &mut crate::cpu::ControlContext,
        arch_context: &mut crate::cpu::ArchContext,
    ) {
        use crate::memory::VmemRegister;

        const PRIO_TIME_SLICE_MULTIPLIER: u16 = 10;

        debug_assert!(!crate::interrupts::are_enabled());

        // Move the current task, if any, back into the scheduler queue.
        if let Some(mut cur_task) = self.cur_task.take() {
            cur_task.ctrl_flow_context = *ctrl_flow_context;
            cur_task.arch_context = *arch_context;
            cur_task.root_page_table_args = VmemRegister::read();

            self.push_task(cur_task);
        }

        {
            let mut waiting_tasks = WAITING_TASKS.lock();
            if waiting_tasks.len() > 0 && let Some(new_task) = waiting_tasks.pop_front() {
                self.push_task(new_task);
            }
        }

        unsafe {
            let next_wait_multiplier = if let Some(next_task) = self.pop_task() {
                // Modify interrupt contexts (usually, the registers).
                *ctrl_flow_context = next_task.ctrl_flow_context;
                *arch_context = next_task.arch_context;

                // Set current page tables.
                VmemRegister::write(&next_task.root_page_table_args);

                let next_timer_ms = (next_task.priority().get() as u16) * PRIO_TIME_SLICE_MULTIPLIER;
                self.cur_task = Some(next_task);

                next_timer_ms
            } else {
                let default_task = &self.idle_task;

                // Modify interrupt contexts (usually, the registers).
                *ctrl_flow_context = default_task.ctrl_flow_context;
                *arch_context = default_task.arch_context;

                // Set current page tables.
                VmemRegister::write(&default_task.root_page_table_args);

                PRIO_TIME_SLICE_MULTIPLIER
            };

            crate::local_state::preemption_wait(core::num::NonZeroU16::new(next_wait_multiplier).unwrap());
        }
    }
}
