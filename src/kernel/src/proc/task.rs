use crate::memory::Stack;
use uuid::Uuid;

type EntryPoint = fn() -> u32;

/// Representation object for different contexts of execution in the CPU.
pub struct Task {
    handle: Uuid,
    prio: u8,
    last_run: u32,
    stack: Stack,
    //pcid: Option<PCID>,
    pub ctrl_flow_context: crate::cpu::ControlContext,
    pub arch_context: crate::cpu::ArchContext,
}

// TODO safety
unsafe impl Send for Task {}

impl Task {
    pub fn new(
        priority: u8,
        entry: EntryPoint,
        stack: Stack,
        arch_context: crate::cpu::ArchContext,
        root_page_table_args: crate::memory::PagingRegister,
    ) -> Self {
        // ### Safety: Stack pointer is valid for its length.
        let sp = unsafe { stack.as_ptr().add(stack.len() & !0xF).addr() } as u64;

        Self {
            handle: uuid::Uuid::new_v4(),
            prio: priority,
            last_run: 0,
            stack,
            ctrl_flow_context: crate::cpu::ControlContext { ip: entry as usize as u64, sp },
            arch_context,
        }
    }

    /// Returns this task's ID.
    #[inline]
    pub const fn uuid(&self) -> Uuid {
        self.handle
    }

    /// Returns the [`TaskPriority`] struct for this task.
    #[inline]
    pub const fn priority(&self) -> u8 {
        self.prio
    }

    #[inline]
    pub const fn last_run(&self) -> u32 {
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
