use alloc::{boxed::Box, collections::VecDeque};
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};
use crossbeam_queue::SegQueue;
use libkernel::{
    instructions::tlb::pcid::*,
    memory::StackAlignedBox,
    registers::{control::CR3Flags, RFlags},
    Address, Physical,
};
use x86_64::registers::segmentation::SegmentSelector;

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ThreadRegisters {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

impl ThreadRegisters {
    pub const fn empty() -> Self {
        Self {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct TaskPriority(u8);

impl TaskPriority {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 16;

    #[inline(always)]
    pub const fn new(priority: u8) -> Option<Self> {
        match priority {
            Self::MIN..=Self::MAX => Some(Self(priority)),
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
    //pcid: Option<PCID>,
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
                libkernel::memory::stack_aligned_allocator(),
            ),

            TaskStackOption::AllocateSized(len) => StackAlignedBox::new_uninit_slice_in(
                len,
                libkernel::memory::stack_aligned_allocator(),
            ),

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

pub static mut GLOBAL_TASK_QUEUE: SegQueue<Task> = SegQueue::new();

pub struct Scheduler {
    enabled: AtomicBool,
    tasks: SegQueue<Task>,
    total_priority: AtomicU64,
}

impl Scheduler {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            tasks: SegQueue::new(),
            total_priority: AtomicU64::new(0),
        }
    }

    /// Enables the scheduler to pop tasks.
    #[inline(always)]
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    /// Disables scheduler from popping tasks.
    ///
    /// REMARK: Any task pops which are already in-flight will not be cancelled.
    #[inline(always)]
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    /// Pushes a new task to the scheduling queue.
    pub fn push_task(&self, task: Task) {
        self.total_priority
            .fetch_add(task.prio().get() as u64, Ordering::Relaxed);
        self.tasks.push(task);
    }

    /// If the scheduler is enabled, attempts to return a new task from
    /// the task queue. Returns `None` if the queue is empty.
    pub fn pop_task(&self) -> Option<Task> {
        match self.enabled.load(Ordering::Relaxed) {
            true => self.tasks.pop().map(|task| {
                self.total_priority
                    .fetch_sub(task.prio().get() as u64, Ordering::Relaxed);

                task
            }),
            false => None,
        }
    }

    pub fn get_avg_prio(&self) -> u64 {
        self.total_priority
            .load(Ordering::Relaxed)
            .checked_div(self.tasks.len() as u64)
            .unwrap_or(0)
    }
}
