use crate::memory::Stack;
use alloc::{collections::BinaryHeap, vec::Vec};
use core::sync::atomic::AtomicU64;

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);

pub enum EntryPoint {
    Address(lzstd::Address<lzstd::Virtual>),
    Function(fn() -> !),
}

static WAITING_TASKS: spin::Mutex<Vec<Task>> = spin::Mutex::new(Vec::new());

pub fn queue_task(new_task: Task) {
    crate::interrupts::without(|| {
        let mut waiting_tasks = WAITING_TASKS.lock();
        waiting_tasks.push(new_task);
    })
}

// TODO move `Task` and its types / impls to a module
/// Representation object for different contexts of execution in the CPU.
pub struct Task {
    id: u64,
    prio: u8,
    last_run: u32,
    stack: Stack,
    //pcid: Option<PCID>,
    pub ctrl_flow_context: crate::cpu::ControlContext,
    pub arch_context: crate::cpu::ArchContext,
    pub root_page_table_args: crate::memory::VmemRegister,
}

// TODO safety
unsafe impl Send for Task {}

impl Task {
    pub fn new(
        priority: u8,
        start: EntryPoint,
        stack: Stack,
        arch_context: crate::cpu::ArchContext,
        root_page_table_args: crate::memory::VmemRegister,
    ) -> Self {
        // ### Safety: Stack pointer is valid for its length.
        let sp = unsafe { stack.as_ptr().add(stack.len() & !0xF).addr() } as u64;

        Self {
            id: NEXT_THREAD_ID.fetch_add(1, core::sync::atomic::Ordering::AcqRel),
            prio: priority,
            last_run: 0,
            stack,
            ctrl_flow_context: crate::cpu::ControlContext {
                ip: match start {
                    EntryPoint::Address(address) => address.as_u64(),
                    EntryPoint::Function(function) => function as usize as u64,
                },
                sp,
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

    pub fn last_run(&self) -> u32 {
        self.last_run
    }
}

impl Ord for Task {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering;

        let last_run_cmp = self.last_run().cmp(&other.last_run());
        let priority_cmp = self.priority().cmp(&other.priority());

        match (priority_cmp, last_run_cmp) {
            (Ordering::Greater, Ordering::Greater) => Ordering::Greater,
            (Ordering::Greater, _) => Ordering::Less,
            (Ordering::Equal | Ordering::Less, ordering) => ordering,
        }
    }
}

impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Task {}

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.priority() == other.priority() && self.last_run() == other.last_run()
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
        use crate::memory::VmemRegister;

        const TIME_SLICE: u16 = 5;

        debug_assert!(!crate::interrupts::are_enabled());

        // Move the current task, if any, back into the scheduler queue.
        if let Some(mut cur_task) = self.cur_task.take() {
            cur_task.ctrl_flow_context = *ctrl_flow_context;
            cur_task.arch_context = *arch_context;
            cur_task.root_page_table_args = VmemRegister::read();

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
                VmemRegister::write(&next_task.root_page_table_args);

                self.cur_task = Some(next_task);
            } else {
                let default_task = &self.idle_task;

                // Modify interrupt contexts (usually, the registers).
                *ctrl_flow_context = default_task.ctrl_flow_context;
                *arch_context = default_task.arch_context;

                // Set current page tables.
                VmemRegister::write(&default_task.root_page_table_args);
            };

            crate::local_state::preemption_wait(core::num::NonZeroU16::new_unchecked(TIME_SLICE));
        }
    }
}
