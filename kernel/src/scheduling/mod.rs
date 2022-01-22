use core::{ops::Add, sync::atomic::AtomicU64};

use alloc::{
    boxed::Box,
    collections::{BTreeSet, BinaryHeap},
    vec::Vec,
};
use libstd::{registers::RFlags, structures::idt::InterruptStackFrame, IndexRing};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TaskRegisters {
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

impl TaskRegisters {
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

static CURRENT_TASK_ID: AtomicU64 = AtomicU64::new(0);

pub struct Task {
    id: u64,
    exec_decr: u64,
    pub rip: u64,
    pub cs: u64,
    pub rsp: u64,
    pub ss: u64,
    pub rfl: u64,
    pub gprs: TaskRegisters,
    pub stack: Box<[u8]>,
}

impl Task {
    const DEFAULT_STACK_SIZE: usize = 0x1000;

    pub fn new(function: fn(), stack: Option<Box<[u8]>>, flags: Option<RFlags>) -> Self {
        let rip = function as u64;
        let stack = stack.unwrap_or_else(|| unsafe {
            libstd::memory::malloc::try_get()
                .unwrap()
                .alloc(0x1000, None)
                .unwrap()
                .into_slice()
        });

        use libstd::structures::gdt;
        Self {
            id: CURRENT_TASK_ID.fetch_add(1, core::sync::atomic::Ordering::AcqRel),
            exec_decr: u64::MAX,
            rip,
            cs: gdt::code() as u64,
            rsp: unsafe { stack.as_ptr().add(stack.len()) as u64 },
            ss: gdt::data() as u64,
            rfl: flags.unwrap_or(RFlags::minimal()),
            gprs: TaskRegisters::empty(),
            stack,
        }
    }
}

impl Ord for Task {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.exec_decr.cmp(&other.exec_decr)
    }
}

impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.exec_decr.partial_cmp(&other.exec_decr)
    }
}

struct Tasks {
    priority_index: IndexRing,
    tasks: [BinaryHeap<Task>; 256],
}

impl Tasks {
    pub fn new() -> Self {
        Self {
            priority_index: IndexRing::new(256),
            tasks: [BinaryHeap::new(); 256],
        }
    }

    pub fn get_next(&mut self) -> Option<(u8, Task)> {
        let index = self.priority_index.index();
        let task = self.tasks[index].pop();
        self.priority_index.increment();

        task.map(|task| (index, task))
    }

    pub fn push(&mut self, priority: u8, task: Task) {
        self.tasks[priority as usize].push(task);
    }
}

pub struct Thread {
    pending_tasks: Vec<(u8, Task)>,
    current_task: Option<(u8, Task)>,
    tasks: Tasks,
}

impl Thread {
    pub fn new() -> Self {
        Self {
            pending_tasks: Vec::new(),
            current_task: None,
            tasks: Tasks::new(),
        }
    }

    pub fn queue_task(&mut self, task: Task, priority: u8) {
        self.pending_tasks.push((priority, task))
    }

    // TODO this needs to be a decision, not an always-switch
    pub fn next_task(
        &mut self,
        stack_frame: &mut InterruptStackFrame,
        cached_regs: &mut TaskRegisters,
    ) {
        // Run all pending tasks.
        for (priority, task) in self.pending_tasks.drain(0..) {
            self.tasks.push(priority, task);
        }

        // Move out old task.
        if let Some((priority, task)) = self.current_task.take() {
            task.rip = stack_frame.instruction_pointer.as_u64();
            task.cs = stack_frame.code_segment;
            task.rsp = stack_frame.stack_pointer.as_u64();
            task.ss = stack_frame.stack_segment;
            task.rfl = stack_frame.cpu_flags;
            task.gprs = *cached_regs;
            task.exec_decr -= 1;
            self.tasks.push(priority, task);
        }

        // Move in new task.
        if let Some(next_task) = self.tasks.get_next() {
            unsafe {
                stack_frame
                    .as_mut()
                    .write(x86_64::structures::idt::InterruptStackFrameValue {
                        instruction_pointer: VirtAddr::new_truncate(next_task.1.rip),
                        code_segment: next_task.1.cs,
                        cpu_flags: next_task.1.rfl,
                        stack_pointer: VirtAddr::new_truncate(next_task.1.rsp),
                        stack_segment: next_task.1.ss,
                    })
            };

            *cached_regs = next_task.1.gprs;

            self.current_task = next_task;
        }
    }
}
