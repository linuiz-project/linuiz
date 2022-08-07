use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
use crossbeam_queue::SegQueue;
use libkernel::{
    interrupts::GeneralRegisters,
    memory::PageAlignedBox,
    registers::x64::{control::CR3Flags, RFlags},
    Address, Physical,
};
use x86_64::registers::segmentation::SegmentSelector;

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
    pub fn get(&self) -> u8 {
        self.0
    }
}

/// Represents a stack allocation strategy for a [`Task`].
pub enum TaskStackOption {
    Auto,
    Pages(usize),
}

/// Representation object for different contexts of execution in the CPU.
pub struct Task {
    id: u64,
    prio: TaskPriority,
    //pcid: Option<PCID>,
    pub rip: u64,
    pub cs: u16,
    pub rsp: u64,
    pub ss: u16,
    pub rfl: RFlags,
    pub gprs: GeneralRegisters,
    pub stack: PageAlignedBox<[MaybeUninit<u8>]>,
    pub cr3: (Address<Physical>, CR3Flags),
}

impl Task {
    const DEFAULT_STACK_SIZE: usize = 0x5000;

    pub fn new(
        priority: TaskPriority,
        function: fn() -> !,
        stack: TaskStackOption,
        rfl: RFlags,
        cs: SegmentSelector,
        ss: SegmentSelector,
        cr3: (Address<Physical>, CR3Flags),
    ) -> Self {
        let rip = function as u64;

        let stack = match stack {
            TaskStackOption::Auto => PageAlignedBox::new_uninit_slice_in(
                Self::DEFAULT_STACK_SIZE,
                libkernel::memory::page_aligned_allocator(),
            ),

            TaskStackOption::Pages(page_count) => PageAlignedBox::new_uninit_slice_in(
                (page_count + 1) * 0x1000,
                libkernel::memory::page_aligned_allocator(),
            ),
        };

        // Unmap stack canary.
        crate::memory::get_kernel_page_manager()
            .unwrap()
            .unmap(
                &libkernel::memory::Page::from_ptr(stack.as_ptr()),
                libkernel::memory::FrameOwnership::None,
                crate::memory::get_kernel_frame_manager().unwrap(),
            )
            .unwrap();

        Self {
            id: NEXT_THREAD_ID.fetch_add(1, core::sync::atomic::Ordering::AcqRel),
            prio: priority,
            rip,
            cs: cs.0,
            rsp: unsafe { stack.as_ptr().add(stack.len()) as u64 },
            ss: ss.0,
            rfl,
            gprs: ThreadRegisters::empty(),
            stack,
            cr3,
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

pub static mut GLOBAL_TASK_QUEUE: SegQueue<Task> = SegQueue::new();

pub struct Scheduler {
    enabled: AtomicBool,
    tasks: SegQueue<Task>,
    total_priority: AtomicU64,
}

impl Scheduler {
    pub fn new(enabled: bool) -> Self {
        Self { enabled: AtomicBool::new(enabled), tasks: SegQueue::new(), total_priority: AtomicU64::new(0) }
    }

    /// Enables the scheduler to pop tasks.
    #[inline]
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    /// Disables scheduler from popping tasks.
    ///
    /// REMARK: Any task pops which are already in-flight will not be cancelled.
    #[inline]
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    /// Indicates whether the scheduler is enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Pushes a new task to the scheduling queue.
    pub fn push_task(&self, task: Task) {
        self.total_priority.fetch_add(task.priority().get() as u64, Ordering::Relaxed);
        self.tasks.push(task);
    }

    /// If the scheduler is enabled, attempts to return a new task from
    /// the task queue. Returns `None` if the queue is empty.
    pub fn pop_task(&self) -> Option<Task> {
        match self.enabled.load(Ordering::Relaxed) {
            true => self.tasks.pop().map(|task| {
                self.total_priority.fetch_sub(task.priority().get() as u64, Ordering::Relaxed);

                task
            }),
            false => None,
        }
    }

    pub fn get_avg_prio(&self) -> u64 {
        self.total_priority.load(Ordering::Relaxed).checked_div(self.tasks.len() as u64).unwrap_or(0)
    }
}
