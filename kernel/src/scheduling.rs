use crate::{clock::local::Stopwatch, tables::idt::InterruptStackFrame};
use alloc::{boxed::Box, collections::BinaryHeap};
use core::{cmp, sync::atomic::AtomicU64};
use lib::{
    registers::{control::CR3Flags, RFlags},
    Address, Physical,
};

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

static CURRENT_THREAD_ID: AtomicU64 = AtomicU64::new(0);

pub struct Thread {
    id: u64,
    prio: u8,
    time: Stopwatch,
    pub rip: u64,
    pub cs: u16,
    pub rsp: u64,
    pub ss: u16,
    pub rfl: RFlags,
    pub gprs: ThreadRegisters,
    pub stack: Box<[u8]>,
    pub cr3: (Address<Physical>, CR3Flags),
}

impl Thread {
    const DEFAULT_STACK_SIZE: usize = 0x1000;

    pub fn new(
        priority: u8,
        function: fn() -> !,
        stack: Option<Box<[u8]>>,
        flags: Option<RFlags>,
        cr3: (Address<Physical>, CR3Flags),
    ) -> Self {
        let rip = function as u64;
        let stack = stack.unwrap_or_else(|| unsafe {
            lib::memory::malloc::get()
                .alloc(Self::DEFAULT_STACK_SIZE, None)
                .unwrap()
                .into_slice()
        });

        use crate::tables::gdt;
        Self {
            id: CURRENT_THREAD_ID.fetch_add(1, core::sync::atomic::Ordering::AcqRel),
            prio: priority,
            time: Stopwatch::new(),
            rip,
            cs: gdt::UCODE_SELECTOR.get().unwrap().0,
            rsp: unsafe { stack.as_ptr().add(stack.len()) as u64 },
            ss: gdt::UDATA_SELECTOR.get().unwrap().0,
            rfl: flags.unwrap_or(RFlags::minimal()),
            gprs: ThreadRegisters::empty(),
            stack,
            cr3,
        }
    }
}

impl Eq for Thread {}
impl PartialEq for Thread {
    fn eq(&self, other: &Self) -> bool {
        self.prio == other.prio && self.time.elapsed_ticks() == other.time.elapsed_ticks()
    }
}

impl Ord for Thread {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let self_time = self.time.elapsed_ticks();
        let other_time = other.time.elapsed_ticks();

        if self_time < other_time {
            if self.prio > other.prio {
                cmp::Ordering::Greater
            } else {
                cmp::Ordering::Less
            }
        } else if self_time == other_time {
            self.prio.cmp(&other.prio)
        } else {
            cmp::Ordering::Less
        }
    }
}

impl PartialOrd for Thread {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

const MAX_TIME_SLICE_MS: usize = 50;
const MIN_TIME_SLICE_MS: usize = 4;

pub struct Scheduler {
    enabled: bool,
    tasks: BinaryHeap<Thread>,
    current_task: Option<Thread>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            enabled: false,
            tasks: BinaryHeap::new(),
            current_task: None,
        }
    }

    pub fn push_thread(&mut self, task: Thread) {
        self.tasks.push(task)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    // TODO this needs to be a decision, not an always-switch
    pub fn run_next(
        &mut self,
        stack_frame: &mut InterruptStackFrame,
        cached_regs: *mut ThreadRegisters,
    ) -> usize {
        // Move out old task.
        if let Some(mut task) = self.current_task.take() {
            task.rip = stack_frame.instruction_pointer.as_u64();
            task.cs = stack_frame.code_segment as u16;
            task.rsp = stack_frame.stack_pointer.as_u64();
            task.ss = stack_frame.stack_segment as u16;
            task.rfl = RFlags::from_bits_truncate(stack_frame.cpu_flags);
            task.gprs = unsafe { cached_regs.read_volatile() };
            task.time.stop();

            self.tasks.push(task);
        }

        // Move in new task if enabled.
        if self.enabled {
            if let Some(mut next_task) = self.tasks.pop() {
                unsafe {
                    // Restart the current task timer.
                    next_task.time.restart();

                    // Modify task frame to restore rsp & rip.
                    stack_frame
                        .as_mut()
                        .write(x86_64::structures::idt::InterruptStackFrameValue {
                            instruction_pointer: x86_64::VirtAddr::new_truncate(next_task.rip),
                            code_segment: next_task.cs as u64,
                            cpu_flags: next_task.rfl.bits(),
                            stack_pointer: x86_64::VirtAddr::new_truncate(next_task.rsp),
                            stack_segment: next_task.ss as u64,
                        });

                    // Restore task registers.
                    cached_regs.write_volatile(next_task.gprs);

                    // Set current page tables.
                    lib::registers::control::CR3::write(next_task.cr3.0, next_task.cr3.1);
                }

                self.current_task = Some(next_task);
            }
        }

        // Calculate next one-shot timer value.
        let total_tasks = self.tasks.len() + if self.current_task.is_some() { 1 } else { 0 };
        if total_tasks > 0 {
            (1000 / total_tasks).clamp(MIN_TIME_SLICE_MS, MAX_TIME_SLICE_MS)
        } else {
            // If only one task, no time slice is required.
            0
        }
    }
}
