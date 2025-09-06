use crate::{
    mem::mapper::{Mapper, paging::Depth},
    util::{elf::segment_to_mapping_permissions, sync::Once},
};
use libsys::{
    address::{Address, Frame, Page},
    constants::{huge_page_size, large_page_size, page_size},
};

pub mod addr;
pub mod alloc;
pub mod mapper;
pub mod pmm;

mod address_space;
pub use address_space::*;

mod hhdm;
pub use hhdm::*;

// pub mod io;

#[derive(Debug, Clone, Copy)]
pub enum FrameSize {
    Standard,
    Large,
    Huge,
}

impl FrameSize {
    pub const fn size_in_bytes(self) -> usize {
        match self {
            FrameSize::Standard => page_size(),
            FrameSize::Large => large_page_size(),
            FrameSize::Huge => huge_page_size(),
        }
    }

    pub const fn size_in_frames(self) -> usize {
        self.size_in_bytes() >> page_size()
    }
}

#[derive(Debug)]
pub struct KernelMapper(Mapper);

static KERNEL_MAPPER: Once<KernelMapper> = Once::new();

impl KernelMapper {
    pub fn init(
        memory_map_request: &limine::request::MemoryMapRequest,
        kernel_file_request: &limine::request::ExecutableFileRequest,
        kernel_address_request: &limine::request::ExecutableAddressRequest,
    ) {
        unsafe fn map_range(
            mapper: &mut Mapper,
            from: Address<Page>,
            to: Address<Frame>,
            byte_count: usize,
            memory_access: Permissions,
            lock_frames: bool,
        ) {
            trace!(
                "Map Range (Args): {{ from: {from:X?}, to: {to:X?}, byte_count: {byte_count:#X}, access: {memory_access:?} }}"
            );

            let virtual_start = from.get().get();
            let virtual_end = virtual_start + byte_count;
            debug!(
                "Map Range: ({memory_access:?}): {:#X?} -> {:#X}",
                virtual_start..virtual_end,
                to.get().get()
            );

            let mut remaining_bytes = byte_count;
            while remaining_bytes > 0 {
                let offset = byte_count - remaining_bytes;
                let from = from.get().get() + offset;
                let to = to.get().get() + offset;
                trace!(
                    "Map Range (Loop): {from:#X} -> {to:#X} {{ Remaining: {remaining_bytes:#X} }}"
                );

                let from = Address::<Page>::new(from).unwrap();
                let to = Address::<Frame>::new(to).unwrap();

                if mapper::use_huge_pages()
                    && remaining_bytes >= huge_page_size()
                    && from.get().get().trailing_zeros() >= huge_page_size().trailing_zeros()
                {
                    // Map a huge page

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so is unused.
                    // - Caller is required to ensure `memory_access` is correct.
                    unsafe {
                        mapper
                            .map(from, to, Depth::huge(), lock_frames, memory_access)
                            .expect("failed to map range");
                    }

                    remaining_bytes -= huge_page_size();
                } else if mapper::use_large_pages()
                    && remaining_bytes >= large_page_size()
                    && from.get().get().trailing_zeros() >= large_page_size().trailing_zeros()
                {
                    // Map a large page

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so is unused.
                    // - Caller is required to ensure `memory_access` is correct.
                    unsafe {
                        mapper
                            .map(from, to, Depth::large(), lock_frames, memory_access)
                            .expect("failed to map range");
                    }

                    remaining_bytes -= large_page_size();
                } else {
                    // Map a standard page

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so is unused.
                    // - Caller is required to ensure `memory_access` is correct.
                    unsafe {
                        mapper
                            .map(from, to, Depth::max(), lock_frames, memory_access)
                            .expect("failed to map range");
                    }

                    remaining_bytes = remaining_bytes.saturating_sub(page_size());
                }
            }
        }

        KERNEL_MAPPER.call_once(|| {
            debug!("Preparing kernel memory...");

            debug!(
                "Paging Setup Info: {{\n\
                \tlarge pages: {{ enabled: {}, size: {:#X} }}\n\
                \thuge pages: {{ enabled: {}, size: {:#X} }}\n\
                }}",
                mapper::use_large_pages(),
                large_page_size(),
                mapper::use_huge_pages(),
                huge_page_size()
            );

            let mut kernel_mapper = Mapper::new();

            trace!("Mapping the higher-half direct map...");
            memory_map_request
                .get_response()
                .expect("bootloader did not provide a response to the memory map request")
                .entries()
                .iter()
                .map(|entry| {
                    trace!(
                        "Mapping Entry: {{ start: {:#X}, length: {:#X}, type: {} }}",
                        entry.base,
                        entry.length,
                        crate::limine_memory_map_entry_type_to_str(entry.entry_type)
                    );

                    let entry_start = usize::try_from(entry.base).unwrap();
                    let entry_length = usize::try_from(entry.length).unwrap();
                    let entry_permissions = {
                        match entry.entry_type {
                            limine::memory_map::EntryType::USABLE
                            | limine::memory_map::EntryType::ACPI_NVS
                            | limine::memory_map::EntryType::ACPI_RECLAIMABLE
                            | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
                            | limine::memory_map::EntryType::FRAMEBUFFER => Permissions::ReadWrite,

                            limine::memory_map::EntryType::RESERVED
                            | limine::memory_map::EntryType::EXECUTABLE_AND_MODULES => {
                                Permissions::ReadOnly
                            }

                            _ => {
                                unreachable!(
                                    "Unrecognized memory map entry type: {:#X}",
                                    entry.base
                                )
                            }
                        }
                    };

                    (entry_start, entry_length, entry_permissions)
                })
                .reduce(
                    |(prev_start_address, prev_length, prev_permissions),
                     (start_address, length, permissions)| {
                        let prev_end_address = prev_start_address + prev_length;
                        if prev_end_address == start_address && prev_permissions == permissions {
                            trace!(
                                "Compounding: {:#X?} <=> {:#X?}",
                                prev_start_address..prev_end_address,
                                start_address..(start_address + length)
                            );

                            (prev_start_address, prev_length + length, permissions)
                        } else {
                            let start_frame = Address::<Frame>::new(prev_start_address).unwrap();
                            let start_page = HigherHalfDirectMap::frame_to_page(start_frame);

                            unsafe {
                                map_range(
                                    &mut kernel_mapper,
                                    start_page,
                                    start_frame,
                                    prev_length,
                                    prev_permissions,
                                    false,
                                );
                            }

                            (start_address, length, permissions)
                        }
                    },
                );

            // Extract the kernel file's physical and virtual addresses.
            let (kernel_physical_address, kernel_virtual_address) = kernel_address_request
                .get_response()
                .map(|response| {
                    (
                        usize::try_from(response.physical_base()).unwrap(),
                        usize::try_from(response.virtual_base()).unwrap(),
                    )
                })
                .expect("bootloader did not provide a response to kernel address request");

            trace!("Mapping kernel executable...");
            kernel_file_request
                .get_response()
                .map(limine::response::ExecutableFileResponse::file)
                .map(|kernel_file| {
                    // Safety: Bootloader guarantees the requisite memory region is correct.
                    unsafe {
                        core::slice::from_raw_parts_mut(
                            kernel_file.addr(),
                            usize::try_from(kernel_file.size()).unwrap(),
                        )
                    }
                })
                .map(|kernel_memory| {
                    elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(kernel_memory)
                        .expect("could not parse kernel file into ELF")
                })
                .expect("bootloader did not provide a response to kernel file request")
                .segments()
                .expect("could not get kernel file segments")
                .iter()
                .filter(|program_header| program_header.p_type == elf::abi::PT_LOAD)
                .for_each(|program_header| {
                    trace!("Kernel Segment: {program_header:X?}");

                    let offset =
                        usize::try_from(program_header.p_vaddr).unwrap() - kernel_virtual_address;
                    let segment_page =
                        Address::<Page>::new(kernel_virtual_address + offset).unwrap();
                    let segment_frame =
                        Address::<Frame>::new(kernel_physical_address + offset).unwrap();
                    // If the segment length is smaller than it's align (for instance with large
                    // page alignments), we want to map based on that.
                    let segment_length = usize::try_from(core::cmp::max(
                        program_header.p_memsz,
                        program_header.p_align,
                    ))
                    .unwrap();
                    let segment_permissions =
                        segment_to_mapping_permissions(program_header.p_flags);

                    unsafe {
                        map_range(
                            &mut kernel_mapper,
                            segment_page,
                            segment_frame,
                            segment_length,
                            segment_permissions,
                            false,
                        );
                    }
                });

            #[cfg(target_arch = "x86_64")]
            {
                let local_apic_frame =
                crate::arch::x86_64::registers::model_specific::IA32_APIC_BASE::get_base_address();

                trace!("Mapping the local APIC: {local_apic_frame:X?}");

                unsafe {
                    map_range(
                        &mut kernel_mapper,
                        HigherHalfDirectMap::frame_to_page(local_apic_frame),
                        local_apic_frame,
                        page_size(),
                        Permissions::ReadWrite,
                        true,
                    );
                }
            }

            let kernel_mapper = Self(kernel_mapper);

            debug!("Kernel mappings complete.");

            kernel_mapper
        });
    }

    fn get_static() -> &'static Self {
        KERNEL_MAPPER.get().unwrap()
    }

    pub fn clone() -> Mapper {
        Self::get_static().0.clone()
    }

    pub unsafe fn swap_into() {
        let mapper = &Self::get_static().0;

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            mapper.swap_into(AddressSpaceId::KERNEL);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permissions {
    None,
    ReadOnly,
    ReadWrite,
    ReadExecute,
    WriteExecute,
}

/// Zeros the higher-half direct mapped memory of `frame`.
///
/// # Safety
///
/// - The higher-half direct mapped memory of `frame` must not be otherwise
///   aliased.
pub unsafe fn zero_frame(frame: Address<Frame>) {
    let page = HigherHalfDirectMap::frame_to_page(frame);
    let ptr = core::ptr::with_exposed_provenance_mut::<u8>(page.get().get());

    // Safety: Caller is required to maintain safety invariants.
    unsafe {
        core::ptr::write_bytes(ptr, 0, page_size());
    }
}
