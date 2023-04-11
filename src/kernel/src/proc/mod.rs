mod context;
pub use context::*;

mod scheduling;
pub use scheduling::*;

mod address_space;
pub use address_space::*;

use crate::memory::alloc::pmm::PhysicalAllocator;
use core::num::NonZeroUsize;
use spin::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Idle = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

pub type EntryPoint = extern "C" fn(args: &[&core::ffi::CStr]) -> u32;

pub type Context = (State, Registers);

pub struct Process {
    id: Uuid,
    priority: Priority,
    address_space: Mutex<AddressSpace<PhysicalAllocator>>,
    context: Context,
}

impl Process {
    pub fn new(priority: Priority, entry: EntryPoint, mut address_space: AddressSpace<PhysicalAllocator>) -> Self {
        const STACK_PAGES: NonZeroUsize = NonZeroUsize::new(16).unwrap();

        let stack = address_space.map(None, STACK_PAGES, MmapFlags::READ_WRITE).unwrap();

        Self {
            id: uuid::Uuid::new_v4(),
            priority,
            address_space: Mutex::new(address_space),
            context: (
                State {
                    ip: (entry as usize).try_into().unwrap(),
                    sp: unsafe { stack.as_non_null_ptr().as_ptr().add(stack.len()).addr() as u64 },
                },
                Registers::user_default(),
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
