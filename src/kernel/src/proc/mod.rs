mod context;
pub use context::*;

mod scheduling;
pub use scheduling::*;

use crate::memory::{
    address_space::{AddressSpace, MmapFlags},
    alloc::pmm::PhysicalAllocator,
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

pub type EntryPoint = extern "C" fn(args: &[&core::ffi::CStr]) -> u32;

pub struct Process {
    id: Uuid,
    priority: Priority,
    address_space: Mutex<AddressSpace<PhysicalAllocator>>,
    context: Context,
}

impl Process {
    pub fn new(priority: Priority, entry: EntryPoint, address_space: AddressSpace<PhysicalAllocator>) -> Self {
        const STACK_PAGES: NonZeroUsize = NonZeroUsize::new(16).unwrap();

        Self {
            id: uuid::Uuid::new_v4(),
            priority,
            address_space: Mutex::new(address_space),
            context: Context::new(
                State {
                    ip: (entry as usize).try_into().unwrap(),
                    sp: address_space.map(None, STACK_PAGES, MmapFlags::READ_WRITE).unwrap().addr().get() as u64,
                },
                Registers::default(),
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
