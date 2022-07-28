use alloc::boxed::Box;
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
use crossbeam_queue::SegQueue;
use liblz::{
    ThreadRegisters,
    memory::StackAlignedBox,
    registers::{control::CR3Flags, RFlags},
    Address, Physical,
};
use x86_64::registers::segmentation::SegmentSelector;

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct TaskPriority(u8);

impl TaskPriority {
    #[inline(always)]
    pub const fn new(priority: u8) -> Option<Self> {
        match priority {
            1..17 => Some(Self(priority)),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn get(&self) -> u8 {
        self.0
    }
}

pub enum TaskStackOption {
    AutoAllocate,
    AllocateSized(usize),
    Preallocated(StackAlignedBox<[MaybeUninit<u8>]>),
}

pub struct Task {
    id: u64,
    prio: TaskPriority,
    pub rip: u64,
    pub cs: u16,
    pub rsp: u64,
    pub ss: u16,
    pub rfl: RFlags,
    pub gprs: ThreadRegisters,
    pub stack: StackAlignedBox<[MaybeUninit<u8>]>,
    pub cr3: (Address<Physical>, CR3Flags),
}

impl Task {
    const DEFAULT_STACK_SIZE: usize = 0x4000;

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
            TaskStackOption::AutoAllocate => StackAlignedBox::new_uninit_slice_in(
                Self::DEFAULT_STACK_SIZE,
                liblz::memory::stack_aligned_allocator(),
            ),

            TaskStackOption::AllocateSized(len) => {
                StackAlignedBox::new_uninit_slice_in(len, liblz::memory::stack_aligned_allocator())
            }

            TaskStackOption::Preallocated(stack) => stack,
        };

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

    pub fn prio(&self) -> TaskPriority {
        self.prio
    }
}
impl core::fmt::Debug for Task {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Task")
            .field("Priority", &self.prio)
            .finish()
    }
}

lazy_static::lazy_static! {
    pub static ref SCHEDULER: Scheduler = Scheduler {
        enabled: AtomicBool::new(false),
        tasks: SegQueue::new()
    };
}

pub struct Scheduler {
    enabled: AtomicBool,
    tasks: SegQueue<Task>,
}

impl Scheduler {
    /// Enables the scheduler to pop tasks.
    #[inline(always)]
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Release);
    }

    /// Disables scheduler from popping tasks.
    ///
    /// REMARK: Any task pops which are already in-flight will not be cancelled.
    #[inline(always)]
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Release);
    }

    /// Pushes a new task to the scheduling queue.
    pub fn push_task(&self, task: Task) {
        self.tasks.push(task);
    }

    /// If the scheduler is enabled, attempts to return a new task from
    /// the task queue. Returns `None` if the queue is empty.
    pub fn pop_task(&self) -> Option<Task> {
        match self.enabled.load(Ordering::Relaxed) {
            true => self.tasks.pop(),
            false => None,
        }
    }
}
