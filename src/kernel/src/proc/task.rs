use crate::memory::{
    address_space::{AddressSpace, MmapFlags},
    PhysicalAllocator,
};
use core::{num::NonZeroUsize,};
use spin::Mutex;
use uuid::Uuid;

pub type EntryPoint = fn() -> u32;

/// Representation object for different contexts of execution in the CPU.
pub struct Task {
    uuid: Uuid,
    prio: u8,
    last_run: u32,
    //pcid: Option<PCID>,
    address_space: Mutex<AddressSpace<PhysicalAllocator>>,
    pub ctrl_flow_context: crate::cpu::Control,
    pub arch_context: crate::cpu::ArchContext,
}

impl Task {
    pub fn new(
        priority: u8,
        entry: EntryPoint,
        address_space: AddressSpace<PhysicalAllocator>,
        arch_context: crate::cpu::ArchContext,
    ) -> Self {
        const STACK_PAGES: NonZeroUsize = NonZeroUsize::new(16).unwrap();

        Self {
            uuid: uuid::Uuid::new_v4(),
            prio: priority,
            last_run: 0,
            address_space: Mutex::new(address_space),
            ctrl_flow_context: crate::cpu::Control {
                ip: entry as usize as u64,
                sp: {
                    let stack =
                        address_space.m_map(None, NonZeroUsize::new(16).unwrap(), MmapFlags::READ_WRITE).unwrap();

                    // Safety: Stack pointer is valid for its length.
                    unsafe { stack.get_unchecked_mut(stack.len()).addr().get().try_into().unwrap() }
                },
            },
            arch_context,
        }
    }

    /// Returns this task's ID.
    #[inline]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
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

    pub fn with_address_space<T>(&self, with_fn: impl FnOnce(&mut AddressSpace<PhysicalAllocator>) -> T) -> T {
        let mut address_space = self.address_space.lock();
        with_fn(&mut address_space)
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
