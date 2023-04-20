mod context;
use bit_field::BitField;
pub use context::*;

mod scheduling;
pub use scheduling::*;

mod address_space;
pub use address_space::*;

use crate::memory::alloc::AlignedAllocator;
use alloc::{boxed::Box, string::String};
use elf::{endian::AnyEndian, file::FileHeader, segment::ProgramHeader};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Idle = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
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
    elf_header: FileHeader<AnyEndian>,
    elf_segments: Box<[ProgramHeader]>,
    elf_data: ElfData,
}

impl Process {
    pub fn new(
        priority: Priority,
        mut address_space: AddressSpace,
        elf_header: FileHeader<AnyEndian>,
        elf_segments: Box<[ProgramHeader]>,
        elf_data: ElfData,
    ) -> Self {
        const STACK_PAGES: core::num::NonZeroUsize = core::num::NonZeroUsize::new(16).unwrap();

        let stack = address_space.mmap(None, STACK_PAGES, MmapPermissions::ReadWrite).unwrap();

        Self {
            id: uuid::Uuid::new_v4(),
            priority,
            address_space,
            context: (
                State::user(elf_header.e_entry, unsafe {
                    stack.as_non_null_ptr().as_ptr().add(stack.len()).addr() as u64
                }),
                Registers::default(),
            ),
            elf_header,
            elf_segments,
            elf_data,
        }
    }

    #[inline]
    pub const fn uuid(&self) -> uuid::Uuid {
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
}
