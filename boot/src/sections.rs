#![allow(dead_code, unused)]

use core::mem::size_of;

use libstd::elf::{ELFHeader64, Rela64, SectionAttributes, SectionHeader, SectionType};
use uefi::proto::media::file::RegularFile;

struct SectionIterator<'k> {
    file: &'k mut RegularFile,
    header: &'k ELFHeader64,
    section_index: u16,
    section_buffer: [u8; size_of::<SectionHeader>()],
    disk_offset: u64,
}

impl<'k> SectionIterator<'k> {
    fn new(file: &'k mut RegularFile, header: &'k ELFHeader64) -> Self {
        Self {
            file,
            header,
            section_index: 0,
            section_buffer: [0u8; size_of::<SectionHeader>()],
            disk_offset: header.section_headers_offset() as u64,
        }
    }
}

impl<'k> Iterator for SectionIterator<'k> {
    type Item = &'k SectionHeader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.section_index < self.header.section_header_count() {
            crate::read_file(self.file, self.disk_offset, &mut self.section_buffer);
            self.section_index += 1;
            self.disk_offset += self.header.section_header_size() as u64;

            Some(unsafe {
                (self.section_buffer.as_ptr() as *const Self::Item)
                    .as_ref()
                    .unwrap()
            })
        } else {
            None
        }
    }
}

#[warn(deprecated)]
fn allocate_sections(
    boot_services: &uefi::prelude::BootServices,
    kernel_file: &mut RegularFile,
    kernel_header: &ELFHeader64,
) {
    let sections: alloc::vec::Vec<SectionHeader> = SectionIterator::new(kernel_file, kernel_header)
        .map(|section| section.clone())
        .collect();

    let total_memory_size = sections
        .iter()
        .filter_map(|section| {
            if section.attribs.contains(SectionAttributes::ALLOC) {
                Some(libstd::align_up(section.size, section.addr_align))
            } else {
                None
            }
        })
        .sum::<usize>();

    let buffer = crate::allocate_pages(
        boot_services,
        uefi::table::boot::AllocateType::Address(0x0),
        uefi::table::boot::MemoryType::RESERVED,
        libstd::align_up_div(total_memory_size, 0x1000),
    );
    let buffer_base = buffer.as_ptr() as usize;

    for section_header in sections.iter() {
        match section_header.ty {
            SectionType::PROGBITS => crate::read_file(
                kernel_file,
                section_header.offset as u64,
                &mut buffer[(section_header.addr.as_usize())..],
            ),
            SectionType::NOBITS => {
                let base = section_header.addr.as_usize();
                buffer[base..(base + section_header.size)].fill(0)
            }
            SectionType::RELA => {
                assert_eq!(
                    section_header.entry_size,
                    size_of::<Rela64>(),
                    "Unknown entry size for RELA section: {}",
                    section_header.entry_size
                );

                let mut rela_buffer = [0u8; size_of::<Rela64>()];
                for offset in (0..section_header.size).step_by(section_header.entry_size) {
                    crate::read_file(kernel_file, section_header.offset as u64, &mut rela_buffer);
                    let rela: &Rela64 = unsafe { &core::mem::transmute(rela_buffer) };

                    debug!("Processing relocation: {:?}", rela);

                    if rela.info == libstd::elf::X86_64_RELATIVE
                        && (buffer_base..=(buffer_base + total_memory_size))
                            .contains(&rela.addr.as_usize())
                    {
                        unsafe {
                            buffer
                                .as_mut_ptr()
                                .add(rela.addr.as_usize())
                                .cast::<u64>()
                                .write(rela.addend)
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
