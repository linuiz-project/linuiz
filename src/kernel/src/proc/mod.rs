mod context;
pub use context::*;

mod scheduling;
pub use scheduling::*;

pub mod task;

use alloc::collections::VecDeque;

pub static TASKS: spin::Mutex<VecDeque<Process>> = spin::Mutex::new(VecDeque::new());

use crate::memory::{
    address_space::{AddressSpace, MmapFlags},
    PhysicalAllocator,
};
use core::num::NonZeroUsize;
use spin::Mutex;
use uuid::Uuid;

pub enum Priority {
    Idle = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

pub type ProcessEntry = extern "C" fn(args: &[&core::ffi::CStr]) -> u32;

pub struct Process {
    id: Uuid,
    priority: Priority,
    address_space: Mutex<AddressSpace<PhysicalAllocator>>,
    context: Context,
}

impl Process {
    pub fn new(
        priority: Priority,
        entry: ProcessEntry,
        regsiters: Option<Registers>,
        address_space: AddressSpace<PhysicalAllocator>,
    ) -> Self {
        const STACK_PAGES: NonZeroUsize = NonZeroUsize::new(16).unwrap();

        Self {
            id: uuid::Uuid::new_v4(),
            priority,
            address_space: Mutex::new(address_space),
            context: Context::new(
                State {
                    ip: (entry as usize).try_into().unwrap(),
                    sp: address_space.m_map(None, STACK_PAGES, MmapFlags::READ_WRITE).unwrap().addr().get() as u64,
                },
                regsiters,
            ),
        }
    }

    #[inline]
    pub const fn uuid(&self) -> Uuid {
        self.id
    }

    #[inline]
    pub const fn priority(&self) -> Priority {
        self.priority
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
