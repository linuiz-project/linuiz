use alloc::collections::VecDeque;
use core::sync::atomic::AtomicU64;

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct TaskPriority(u8);

impl TaskPriority {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 16;

    /// Safely constructs a new instance of this type, with range checking for the priority.
    #[inline(always)]
    pub const fn new(priority: u8) -> Option<Self> {
        match priority {
            Self::MIN..=Self::MAX => Some(Self(priority)),
            _ => None,
        }
    }

    /// Gets the inner raw priority value.
    #[inline(always)]
    pub fn get(self) -> u8 {
        self.0
    }
}

/// Represents a stack allocation strategy for a [`Task`].
pub enum TaskStack {
    None,
    At(libcommon::Address<libcommon::Virtual>),
}

pub enum TaskStart {
    Address(libcommon::Address<libcommon::Virtual>),
    Function(fn() -> !),
}

// TODO devise a better method for tasks to be queued globally
pub static GLOBAL_TASKS: spin::Lazy<spin::Mutex<VecDeque<Task>>> =
    spin::Lazy::new(|| spin::Mutex::new(VecDeque::new()));

// TODO move `Task` and its types / impls to a module
/// Representation object for different contexts of execution in the CPU.
pub struct Task {
    id: u64,
    prio: TaskPriority,
    //pcid: Option<PCID>,
    pub ctrl_flow_context: libarch::interrupts::ControlFlowContext,
    pub arch_context: libarch::interrupts::ArchContext,
    pub root_page_table_args: libarch::memory::VmemRegister,
}

impl Task {
    pub fn new(
        priority: TaskPriority,
        start: TaskStart,
        stack: TaskStack,
        arch_context: libarch::interrupts::ArchContext,
        root_page_table_args: libarch::memory::VmemRegister,
    ) -> Self {
        Self {
            id: NEXT_THREAD_ID.fetch_add(1, core::sync::atomic::Ordering::AcqRel),
            prio: priority,
            ctrl_flow_context: libarch::interrupts::ControlFlowContext {
                ip: match start {
                    TaskStart::Address(address) => address.as_u64(),
                    TaskStart::Function(function) => function as usize as u64,
                },
                sp: match stack {
                    TaskStack::None => 0x0,
                    TaskStack::At(address) => address.as_u64(),
                },
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
    pub fn priority(&self) -> TaskPriority {
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
    tasks: VecDeque<Task>,
    idle_task: Task,
    cur_task: Option<Task>,
}

impl Scheduler {
    pub fn new(enabled: bool, idle_task: Task) -> Self {
        Self { enabled, total_priority: 0, tasks: VecDeque::new(), idle_task, cur_task: None }
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
        self.tasks.push_back(task);
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
        ctrl_flow_context: &mut libarch::interrupts::ControlFlowContext,
        arch_context: &mut libarch::interrupts::ArchContext,
    ) {
        use libarch::memory::VmemRegister;

        const PRIO_TIME_SLICE_MULTIPLIER: u16 = 10;

        if let Some(mut global_tasks) = GLOBAL_TASKS.try_lock()
            && let Some(task) = global_tasks.pop_front() {
            self.push_task(task);
        }

        // Move the current task, if any, back into the scheduler queue.
        if let Some(mut cur_task) = self.cur_task.take() {
            cur_task.ctrl_flow_context = *ctrl_flow_context;
            cur_task.arch_context = *arch_context;
            cur_task.root_page_table_args = VmemRegister::read();

            self.push_task(cur_task);
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

            debug_assert!(next_wait_multiplier > 0);

            crate::local_state::preemption_wait(next_wait_multiplier);
        }
    }
}
