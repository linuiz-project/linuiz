use crate::{
    default_display_impl,
    memory::{address_space::mapper::Mapper, paging},
    proc::task::EntryPoint,
};
use elf::{endian::AnyEndian, ElfBytes};
use libsys::Address;

const P_FLAG_EXECUTE: u32 = 1 << 0;
const P_FLAG_WRITE: u32 = 1 << 1;

#[derive(Debug)]
pub enum Error {
    NoSegments,
    Paging(paging::Error),
}

default_display_impl!(Error);

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::NoSegments => None,
            Self::Paging(err) => Some(err),
        }
    }
}

impl From<paging::Error> for Error {
    fn from(value: paging::Error) -> Self {
        Self::Paging(value)
    }
}

pub struct Artifact {
    // TODO
    // elf: ElfBytes<'a, AnyEndian>,
    mapper: Mapper,
    entry: EntryPoint,
}

impl Artifact {
    pub fn decompose(self) -> (EntryPoint, Mapper) {
        (self.entry, self.mapper)
    }
}

impl<'a> TryFrom<&ElfBytes<'a, AnyEndian>> for Artifact {
    type Error = Error;

    fn try_from(elf: &ElfBytes<AnyEndian>) -> Result<Self, Self::Error> {
        use crate::memory::{new_kmapped_page_table, PageDepth};
        use libsys::{page_shift, page_size};

        // Attempt to get the segments from the ELF, or fail fast if there are none.
        let Some(segments) = elf.segments()
        else {
            return Err(Error::NoSegments)
        };

        // Create the driver's page manager from the kernel's higher-half table.
        // Safety: Provided depth and frames are valid for use in a mapper.
        let mut mapper = unsafe { Mapper::new_unsafe(PageDepth::new(4), new_kmapped_page_table().unwrap()) };

        // Parse loadable segments.
        for phdr in segments.iter().filter(|phdr| phdr.p_type == elf::abi::PT_LOAD) {
            trace!("{:?}", phdr);

            let memory_size = usize::try_from(phdr.p_memsz).unwrap();
            let memory_start = usize::try_from(phdr.p_vaddr).unwrap();
            let memory_end = memory_start + memory_size;

            // Align the start address to ensure we iterate page-aligned addresses.
            let memory_start_aligned = libsys::align_down(memory_start, page_shift());
            for page_base in (memory_start_aligned..memory_end).step_by(page_size()) {
                let attributes = {
                    // This doesn't support RWX pages. I'm not sure it ever should.
                    if (phdr.p_flags & P_FLAG_EXECUTE) > 0 {
                        paging::Attributes::RX
                    } else if (phdr.p_flags & P_FLAG_WRITE) > 0 {
                        paging::Attributes::RW
                    } else {
                        paging::Attributes::RO
                    }
                };

                let page = Address::new(page_base).unwrap();
                trace!("auto map {:X?}", page);

                mapper.auto_map(page, attributes).map_err(Error::from)?;
            }

            let segment_slice = elf.segment_data(&phdr).unwrap();
            // Safety: `memory_start` pointer is valid as we just mapped all of the requisite pages for `memory_size` length.
            let memory_slice = unsafe { core::slice::from_raw_parts_mut(memory_start as *mut u8, memory_size) };
            // Copy segment data into the new memory region.
            memory_slice[..segment_slice.len()].copy_from_slice(segment_slice);
            // Clear any left over bytes to 0. This is useful for the bss region, for example.
            memory_slice[segment_slice.len()..].fill(0x0);
        }

        Ok(Self { mapper, entry: unsafe { core::mem::transmute(elf.ehdr.e_entry) } })
    }
}
