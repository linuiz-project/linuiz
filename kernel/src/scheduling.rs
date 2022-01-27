use alloc::{boxed::Box, collections::BinaryHeap};
use core::{cmp, sync::atomic::AtomicU64};
use lib::{registers::RFlags, structures::idt::InterruptStackFrame};

#[repr(C, packed)]
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
    prio: u8,
    time: u64,
    pub rip: u64,
    pub cs: u64,
    pub rsp: u64,
    pub ss: u64,
    pub rfl: RFlags,
    pub gprs: TaskRegisters,
    pub stack: Box<[u8]>,
}

impl Task {
    const DEFAULT_STACK_SIZE: usize = 0x1000;

    pub fn new(
        priority: u8,
        function: fn() -> !,
        stack: Option<Box<[u8]>>,
        flags: Option<RFlags>,
    ) -> Self {
        let rip = function as u64;
        let stack = stack.unwrap_or_else(|| unsafe {

            lib::memory::malloc::get()
                .alloc(Self::DEFAULT_STACK_SIZE, None)
                .unwrap()
                .into_slice()
        });

        use lib::structures::gdt;
        Self {
            id: CURRENT_TASK_ID.fetch_add(1, core::sync::atomic::Ordering::AcqRel),
            prio: priority,
            time: u64::MAX,
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

impl Eq for Task {}
impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.prio == other.prio && self.time == other.time
    }
}

impl Ord for Task {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        if self.time < other.time {
            if self.prio > other.prio {
                cmp::Ordering::Greater
            } else {
                cmp::Ordering::Less
            }
        } else if self.time == other.time {
            self.prio.cmp(&other.prio)
        } else {
            cmp::Ordering::Less
        }
    }
}

impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

const MAX_TIME_SLICE_MS: usize = 50;
const MIN_TIME_SLICE_MS: usize = 4;

pub struct Thread {
    enabled: bool,
    tasks: BinaryHeap<Task>,
    current_task: Option<Task>,
}

impl Thread {
    pub fn new() -> Self {
        Self {
            enabled: false,
            tasks: BinaryHeap::new(),
            current_task: None,
        }
    }

    pub fn push_task(&mut self, task: Task) {
        self.tasks.push(task)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    // TODO this needs to be a decision, not an always-switch
    pub fn run_next(
        &mut self,
        stack_frame: &mut InterruptStackFrame,
        cached_regs: *mut TaskRegisters,
    ) -> usize {
        // Move out old task.
        if let Some(mut task) = self.current_task.take() {
            task.rip = stack_frame.instruction_pointer.as_u64();
            task.cs = stack_frame.code_segment;
            task.rsp = stack_frame.stack_pointer.as_u64();
            task.ss = stack_frame.stack_segment;
            task.rfl = RFlags::from_bits_truncate(stack_frame.cpu_flags);
            task.gprs = unsafe { cached_regs.read_volatile() };
            task.time += 1;

            self.tasks.push(task);
        }

        // Move in new task if enabled.
        if self.enabled {
            if let Some(next_task) = self.tasks.pop() {
                unsafe {
                    stack_frame
                        .as_mut()
                        .write(x86_64::structures::idt::InterruptStackFrameValue {
                            instruction_pointer: x86_64::VirtAddr::new_truncate(next_task.rip),
                            code_segment: next_task.cs,
                            cpu_flags: next_task.rfl.bits(),
                            stack_pointer: x86_64::VirtAddr::new_truncate(next_task.rsp),
                            stack_segment: next_task.ss,
                        });
                    cached_regs.write_volatile(next_task.gprs);
                }

                self.current_task = Some(next_task);
            }
        }

        let total_tasks = self.tasks.len() + if self.current_task.is_some() { 1 } else { 0 };
        if total_tasks > 0 {
            (1000 / total_tasks).clamp(MIN_TIME_SLICE_MS, MAX_TIME_SLICE_MS)
        } else {
            // If only one task, no time slice is required.
            0
        }
    }
}
