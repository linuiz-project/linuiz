mod context;

use bit_field::BitField;
pub use context::*;

mod scheduling;
use libsys::{page_size, Address, Virtual};
pub use scheduling::*;

mod address_space;
pub use address_space::*;

use crate::memory::alloc::AlignedAllocator;
use alloc::{boxed::Box, string::String, vec::Vec};
use core::num::NonZeroUsize;
use elf::{endian::AnyEndian, file::FileHeader, segment::ProgramHeader};

pub const STACK_SIZE: NonZeroUsize = NonZeroUsize::new((libsys::MIBIBYTE as usize) - page_size()).unwrap();
pub const STACK_PAGES: NonZeroUsize = NonZeroUsize::new(STACK_SIZE.get() / page_size()).unwrap();
pub const STACK_START: NonZeroUsize = NonZeroUsize::new(page_size()).unwrap();
pub const MIN_LOAD_OFFSET: usize = STACK_START.get() + STACK_SIZE.get();

pub const PT_FLAG_EXEC_BIT: usize = 0;
pub const PT_FLAG_WRITE_BIT: usize = 1;

pub fn segment_type_to_mmap_permissions(segment_ty: u32) -> MmapPermissions {
    if segment_ty.get_bit(PT_FLAG_WRITE_BIT) {
        MmapPermissions::ReadWrite
    } else if segment_ty.get_bit(PT_FLAG_EXEC_BIT) {
        MmapPermissions::ReadExecute
    } else {
        MmapPermissions::ReadOnly
    }
}

pub static PROCESS_BASE: usize = 0x20000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Idle = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

#[derive(Debug, Clone, Copy)]
pub struct ElfRela {
    pub address: Address<Virtual>,
    pub value: usize,
}

pub type Context = (State, Registers);
pub type ElfAllocator = AlignedAllocator<{ libsys::page_size() }>;
pub type ElfMemory = Box<[u8], ElfAllocator>;

pub enum ElfData {
    Memory(ElfMemory),
    File(String),
}

pub struct Process {
    id: uuid::Uuid,
    priority: Priority,

    address_space: AddressSpace,
    context: Context,
    load_offset: usize,

    elf_header: FileHeader<AnyEndian>,
    elf_segments: Box<[ProgramHeader]>,
    elf_relas: Vec<ElfRela>,
    elf_data: ElfData,

    exit: bool,
}

impl Process {
    pub fn new(
        priority: Priority,
        mut address_space: AddressSpace,
        load_offset: usize,
        elf_header: FileHeader<AnyEndian>,
        elf_segments: Box<[ProgramHeader]>,
        elf_relas: Vec<ElfRela>,
        elf_data: ElfData,
    ) -> Self {
        let stack = address_space
            .mmap(Some(Address::new_truncate(STACK_START.get())), STACK_PAGES, MmapPermissions::ReadWrite)
            .unwrap();

        Self {
            id: uuid::Uuid::new_v4(),
            priority,
            address_space,
            context: (
                State::user(u64::try_from(load_offset).unwrap() + elf_header.e_entry, unsafe {
                    stack.as_non_null_ptr().as_ptr().add(stack.len()).addr() as u64
                }),
                Registers::default(),
            ),
            load_offset,
            elf_header,
            elf_segments,
            elf_relas,
            elf_data,

            exit: false,
        }
    }

    #[inline]
    pub const fn id(&self) -> uuid::Uuid {
        self.id
    }

    #[inline]
    pub const fn priority(&self) -> Priority {
        self.priority
    }

    #[inline]
    pub const fn address_space(&self) -> &AddressSpace {
        &self.address_space
    }

    #[inline]
    pub fn address_space_mut(&mut self) -> &mut AddressSpace {
        &mut self.address_space
    }

    #[inline]
    pub const fn load_offset(&self) -> usize {
        self.load_offset
    }

    #[inline]
    pub const fn elf_header(&self) -> &FileHeader<AnyEndian> {
        &self.elf_header
    }

    #[inline]
    pub const fn elf_segments(&self) -> &[ProgramHeader] {
        &self.elf_segments
    }

    #[inline]
    pub const fn elf_data(&self) -> &ElfData {
        &self.elf_data
    }

    #[inline]
    pub fn load_address_to_elf_vaddr(&self, address: Address<Virtual>) -> Option<usize> {
        address.get().checked_sub(self.load_offset)
    }

    #[inline]
    pub fn elf_relas(&mut self) -> &mut Vec<ElfRela> {
        &mut self.elf_relas
    }

    #[inline]
    pub const fn get_exit(&self) -> bool {
        self.exit
    }

    /// ### Safety
    ///
    /// Changing the exit mode of a process is implicitly undefined behaviour, as any
    /// external state the process is modifying may be orphaned.
    #[inline]
    pub unsafe fn set_exit(&mut self, exit: bool) {
        self.exit = exit;
    }
}

#[link_section = ".entry_stub"]
fn _proc_entry_stub(main_fn: extern "sysv64" fn() -> i32) {
    // local process setup

    let _result = main_fn();

    // syscall exit
}
