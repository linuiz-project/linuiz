use crate::{
    memory::Stack,
    proc::{Process, Registers, State},
};
use alloc::collections::VecDeque;

pub static PROCESSES: spin::Mutex<VecDeque<Process>> = spin::Mutex::new(VecDeque::new());

pub struct Scheduler {
    enabled: bool,
    idle_stack: Stack<0x1000>,
    process: Option<Process>,
}

impl Scheduler {
    pub fn new(enabled: bool) -> Self {
        Self { enabled, idle_stack: Stack::new(), process: None }
    }

    /// Enables the scheduler to pop tasks.
    #[inline]
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disables scheduler from popping tasks. Any task pops which are already in-flight will not be cancelled.
    #[inline]
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Indicates whether the scheduler is enabled.
    #[inline]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[inline]
    pub const fn process(&self) -> Option<&Process> {
        self.process.as_ref()
    }

    #[inline]
    pub fn process_mut(&mut self) -> Option<&mut Process> {
        self.process.as_mut()
    }

    /// Attempts to schedule the next task in the local task queue.
    pub fn yield_task(&mut self, state: &mut super::State, regs: &mut super::Registers) {
        debug_assert!(!crate::interrupts::are_enabled());

        let mut processes = PROCESSES.lock();

        // Move the current task, if any, back into the scheduler queue.
        if let Some(mut process) = self.process.take() {
            process.context.0 = *state;
            process.context.1 = *regs;

            trace!("Reclaiming task: {:?}", process.id());
            processes.push_back(process);
        }

        self.next_task(&mut processes, state, regs);
    }

    pub fn exit_task(&mut self, state: &mut super::State, regs: &mut super::Registers) {
        debug_assert!(!crate::interrupts::are_enabled());

        let _ = self.process.take().expect("cannot exit without process");

        let mut processes = PROCESSES.lock();
        self.next_task(&mut processes, state, regs);
    }

    fn next_task(&mut self, processes: &mut VecDeque<Process>, state: &mut State, regs: &mut Registers) {
        // Pop a new task from the task queue, or simply switch in the idle task.
        if let Some(next_process) = processes.pop_front() {
            trace!("Switching task: {:?}", next_process.id());

            *state = next_process.context.0;
            *regs = next_process.context.1;

            // Safety: New task requires its own address space.
            unsafe {
                next_process.address_space.swap_into();
            }

            trace!("Switched task: {:?}", next_process.id());

            let old_value = self.process.replace(next_process);
            debug_assert!(old_value.is_none());
        } else {
            trace!("Switching idle task.");

            *state = State::kernel(crate::interrupts::wait_loop as u64, self.idle_stack.top().addr().get() as u64);
            *regs = Registers::default();

            trace!("Switched idle task.");
        };

        // TODO have some kind of queue of preemption waits, to ensure we select the shortest one.
        // Safety: Just having switched tasks, no preemption wait should supercede this one.
        unsafe {
            const TIME_SLICE: core::num::NonZeroU16 = core::num::NonZeroU16::new(500).unwrap();

            crate::local::set_preemption_wait(TIME_SLICE);
        }
    }
}
