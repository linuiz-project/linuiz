mod context;
use alloc::boxed::Box;
pub use context::*;

mod scheduling;
use elf::{endian::AnyEndian, ElfBytes};
use libsys::page_size;
pub use scheduling::*;

mod address_space;
pub use address_space::*;

use crate::memory::alloc::{pmm::PhysicalAllocator, AlignedAllocator};
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
pub type ElfAllocator = AlignedAllocator<{ page_size() }, PhysicalAllocator>;
pub type ElfData = Box<[u8], ElfAllocator>;

pub struct Process {
    id: Uuid,
    priority: Priority,
    address_space: Mutex<AddressSpace<PhysicalAllocator>>,
    context: Context,
    elf_data: ElfData,
}

impl Process {
    pub fn new(priority: Priority, mut address_space: AddressSpace<PhysicalAllocator>, elf_data: ElfData) -> Self {
        const STACK_PAGES: NonZeroUsize = NonZeroUsize::new(16).unwrap();

        let stack = address_space.mmap(None, STACK_PAGES, false, MmapPermissions::ReadWrite).unwrap();
        let elf = ElfBytes::<AnyEndian>::minimal_parse(&elf_data).unwrap();

        Self {
            id: uuid::Uuid::new_v4(),
            priority,
            address_space: Mutex::new(address_space),
            context: (
                State::user(elf.ehdr.e_entry, unsafe {
                    stack.as_non_null_ptr().as_ptr().add(stack.len()).addr() as u64
                }),
                Registers::default(),
            ),
            elf_data,
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

    pub fn elf(&self) -> ElfBytes<AnyEndian> {
        ElfBytes::minimal_parse(&self.elf_data).unwrap()
    }
}

pub const PT_FLAG_EXEC_BIT: usize = 0;
pub const PT_FLAG_WRITE_BIT: usize = 1;
