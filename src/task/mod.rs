use crate::{arch::x86_64::structures::idt::InterruptStackFrame, mem::Permissions};
use alloc::{boxed::Box, string::String, vec::Vec};
use bit_field::BitField;
use core::{mem::MaybeUninit, num::NonZero, ptr::NonNull};
use elf::{endian::AnyEndian, file::FileHeader, segment::ProgramHeader};
use libsys::{
    address::{Address, Page, Virtual},
    constants::{page_mask, page_size},
};

mod context;
pub use context::*;

mod scheduling;
pub use scheduling::*;

mod address_space;
pub use address_space::*;

pub mod asid;

#[allow(clippy::cast_possible_truncation)]
pub const STACK_SIZE: NonZero<usize> = NonZero::<usize>::new(1_000_000).unwrap();
pub const STACK_PAGES: NonZero<usize> =
    NonZero::<usize>::new(STACK_SIZE.get() / page_size()).unwrap();
pub const STACK_START: NonZero<usize> = NonZero::<usize>::new(page_size()).unwrap();
pub const MIN_LOAD_OFFSET: usize = STACK_START.get() + STACK_SIZE.get();

pub const PT_FLAG_EXEC_BIT: usize = 0;
pub const PT_FLAG_WRITE_BIT: usize = 1;

pub fn segment_to_mapping_permissions(segment_flags: u32) -> Permissions {
    match (
        segment_flags.get_bit(PT_FLAG_WRITE_BIT),
        segment_flags.get_bit(PT_FLAG_EXEC_BIT),
    ) {
        (false, false) => Permissions::ReadOnly,
        (true, false) => Permissions::ReadWrite,
        (false, true) => Permissions::ReadExecute,
        (true, true) => unreachable!("ELF segment is WX"),
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("address is already mapped")]
    AlreadyMapped,

    #[error("tried to demand map a page and underflowed")]
    AddressUnderrun(Address<Virtual>),

    #[error("address belongs to a non-load segment")]
    NonLoadAddress(Address<Virtual>),
}

pub static TASK_LOAD_BASE: usize = 0x20000;

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

pub type Context = (InterruptStackFrame, Registers);

#[derive(Debug)]
pub enum ElfData {
    Memory(Box<[u8]>),
    File(String),
}

pub struct Task {
    id: uuid::Uuid,
    priority: Priority,

    address_space: AddressSpace,
    context: Context,
    load_offset: usize,

    elf_header: FileHeader<AnyEndian>,
    elf_segments: Box<[ProgramHeader]>,
    elf_relas: Vec<ElfRela>,
    elf_data: ElfData,
}

impl Task {
    pub fn new(
        priority: Priority,
        mut address_space: AddressSpace,
        load_offset: usize,
        elf_header: FileHeader<AnyEndian>,
        elf_segments: Box<[ProgramHeader]>,
        elf_relas: Vec<ElfRela>,
        elf_data: ElfData,
    ) -> Self {
        trace!("Generating a random ID for new task.");
        let id = uuid::Uuid::new_v4();

        trace!("Allocating userspace stack for task: {id:?}.");
        let stack = unsafe {
            address_space.mmap(
                MemoryMapping::Any { count: STACK_PAGES },
                Permissions::ReadWrite,
            )
        };
        let stack = stack.unwrap();

        let entry_point = usize::try_from(elf_header.e_entry).unwrap();
        let entry_point = NonZero::<usize>::new(entry_point).unwrap();
        let offset_entry_point = entry_point.checked_add(load_offset).unwrap();
        let instruction_ptr = NonNull::<u8>::with_exposed_provenance(offset_entry_point);

        Self {
            id,
            priority,
            address_space,
            context: (
                InterruptStackFrame::new_user(
                    Some(instruction_ptr),
                    Some({
                        // Safety: Index is the end of the slice.
                        unsafe { stack.get_unchecked_mut(stack.len()) }
                    }),
                ),
                Registers::empty(),
            ),
            load_offset,
            elf_header,
            elf_segments,
            elf_relas,
            elf_data,
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
    pub fn elf_relas(&mut self) -> &mut Vec<ElfRela> {
        &mut self.elf_relas
    }

    #[allow(clippy::too_many_lines)]
    pub fn demand_map(&mut self, address: Address<Virtual>) -> Result<(), Error> {
        let fault_page = Address::<Page>::new_truncate(address.get());

        if self.address_space().is_mmapped(fault_page) {
            return Err(Error::AlreadyMapped);
        }

        let fault_unoffset = address
            .get()
            .checked_sub(self.load_offset())
            .ok_or(Error::AddressUnderrun(address))?;

        let segment = self
            .elf_segments()
            .iter()
            .filter(|phdr| phdr.p_type == elf::abi::PT_LOAD)
            .find(|phdr| {
                (phdr.p_vaddr..(phdr.p_vaddr + phdr.p_memsz))
                    .contains(&u64::try_from(fault_unoffset).unwrap())
            })
            .copied()
            .ok_or(Error::NonLoadAddress(address))?;

        // Small check to help ensure the segment alignments are page-fit.
        debug_assert!(segment.p_align & u64::try_from(page_mask()).unwrap() == 0);

        debug!(
            "Demand mapping {:X?} from segment: {:X?}",
            Address::<Page>::new_truncate(address.get()),
            segment
        );

        let fault_unoffset_page = Address::<Page>::new_truncate(fault_unoffset);
        let fault_unoffset_page_addr = fault_unoffset_page.get().get();

        let fault_unoffset_end_page =
            Address::<Page>::new_truncate(fault_unoffset_page_addr + page_size());
        let fault_unoffset_end_page_addr = fault_unoffset_end_page.get().get();

        let segment_addr = usize::try_from(segment.p_vaddr).unwrap();
        let segment_size = usize::try_from(segment.p_filesz).unwrap();
        let segment_end_addr = segment_addr + segment_size;

        let fault_offset = fault_unoffset_page_addr.saturating_sub(segment_addr);
        let fault_end_pad = fault_unoffset_end_page_addr.saturating_sub(segment_end_addr);
        let fault_front_pad = segment_addr.saturating_sub(fault_unoffset_page_addr);
        let fault_size = ((fault_unoffset_end_page_addr - fault_unoffset_page_addr)
            - fault_front_pad)
            - fault_end_pad;

        trace!("Mapping the demand page RW so data can be copied.");
        let address_space = self.address_space_mut();

        let mapped_memory = unsafe {
            address_space.mmap(
                MemoryMapping::Exact {
                    range: fault_page..fault_page,
                },
                Permissions::ReadWrite,
            )
        };
        let mapped_memory = mapped_memory.unwrap();

        // Safety: Address space allocator fulfills all required invariants.
        let mapped_memory = unsafe { mapped_memory.as_uninit_slice_mut() };

        let (front_pad, remaining) = mapped_memory.split_at_mut(fault_front_pad);
        let (file_memory, end_pad) = remaining.split_at_mut(fault_size);

        debug_assert_eq!(fault_front_pad, front_pad.len(), "mismatch front padding");
        debug_assert_eq!(fault_end_pad, end_pad.len(), "mismatch end padding");
        debug_assert_eq!(fault_size, file_memory.len(), "mismatch file memory");

        trace!(
            "Copying memory into demand mapping: {:#X}..{:#X}..{:#X}.",
            front_pad.len(),
            file_memory.len(),
            end_pad.len()
        );
        front_pad.fill(MaybeUninit::uninit());
        end_pad.fill(MaybeUninit::uninit());

        if !file_memory.is_empty() {
            match self.elf_data() {
                ElfData::Memory(data) => {
                    let segment_data_offset = usize::try_from(segment.p_offset).unwrap();

                    let offset_segment_range = (segment_data_offset + fault_offset)
                        ..(segment_data_offset + fault_offset + fault_size);

                    // Safety: Same-sized reinterpret for copying.
                    let (_, copy_data, _) = unsafe { data[offset_segment_range].align_to() };

                    file_memory.copy_from_slice(copy_data);
                }
                ElfData::File(_) => unimplemented!(),
            }
        }

        trace!("Processing demand mapping relocations.");
        let load_offset = self.load_offset();
        let fault_page_as_range = fault_unoffset_page_addr..fault_unoffset_end_page_addr;

        self.elf_relas().retain(|rela| {
            if fault_page_as_range.contains(&rela.address.get()) {
                trace!("Processing relocation: {rela:X?}");

                let rela_address = NonZero::<usize>::new(rela.address.get() + load_offset).unwrap();
                let rela_ptr = NonNull::<usize>::with_exposed_provenance(rela_address);

                unsafe {
                    rela_ptr.write(rela.value);
                }

                false
            } else {
                true
            }
        });

        trace!("Finalizing page's access attributes.");
        // Safety: Page is already mapped, permissions are being modified according to
        // the segment access type.
        unsafe {
            self.address_space_mut()
                .set_permissions(
                    fault_page,
                    crate::task::segment_to_mapping_permissions(segment.p_type),
                )
                .unwrap();
        }

        trace!("Demand mapping complete.");

        Ok(())
    }
}

impl core::fmt::Debug for Task {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Task")
            .field("ID", &self.id)
            .field("Priority", &self.priority)
            .field("Address Space", &self.address_space)
            .field("Context", &self.context)
            .field("ELF Load Offset", &self.load_offset)
            .field("ELF Header", &self.elf_header)
            .finish_non_exhaustive()
    }
}
