use crate::{
    mem::{
        addr::{
            phys::{FrameAddress, HugeFrame, LargeFrame, StandardFrame},
            virt::{HugePage, LargePage, PageAddress, StandardPage},
        },
        mapper::{Mapper, paging::PageTableInfo},
    },
    util::{elf::segment_to_mapping_permissions, sync::Once},
};
use core::{num::NonZero, ptr::NonNull};

pub mod addr;
pub mod alloc;
pub mod mapper;
pub mod pmm;

mod address_space;
pub use address_space::*;

mod hhdm;
pub use hhdm::*;

// pub mod io;

#[cfg(test)]
pub fn get_paging_depth() -> NonZero<u32> {
    NonZero::new(4).unwrap()
}

#[cfg(not(test))]
pub fn get_paging_depth() -> NonZero<u32> {
    const CR4_LA57_BIT: usize = 1 << 12;

    let cr4: usize;

    unsafe {
        core::arch::asm!(
            "mov {}, cr4",
            out(reg) cr4,
            options(nostack, nomem, preserves_flags)
        );
    }

    if (cr4 & CR4_LA57_BIT) == 0 {
        NonZero::new(4).unwrap()
    } else {
        NonZero::new(5).unwrap()
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
            from: usize,
            to: usize,
            byte_count: usize,
            memory_access: Permissions,
            lock_frames: bool,
        ) {
            debug_assert_eq!((from & StandardFrame::non_index_bit_mask()), 0);
            debug_assert_eq!((to & StandardFrame::non_index_bit_mask()), 0);

            trace!(
                "Map Range (Args): {{ from: {from:X?}, to: {to:X?}, byte_count: {byte_count:#X}, access: {memory_access:?} }}"
            );

            let mut remaining_bytes = byte_count;
            while remaining_bytes > 0 {
                let offset = byte_count - remaining_bytes;
                let from = from + offset;
                let to = to + offset;

                trace!(
                    "Map Range (Loop): {from:#X} -> {to:#X} {{ Remaining: {remaining_bytes:#X} }}"
                );

                if PageTableInfo::is_huge_pages_enabled()
                    && remaining_bytes >= HugeFrame::size_in_bytes()
                    && from.trailing_zeros() >= HugeFrame::size_in_bytes().trailing_zeros()
                    && to.trailing_zeros() >= HugeFrame::size_in_bytes().trailing_zeros()
                {
                    remaining_bytes -= HugeFrame::size_in_bytes();

                    // Map a huge page ...

                    let from = HugePage::new(from).expect("non-canonical overrun");
                    let to = HugeFrame::new(to).expect("non-canonical overrun");

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so is unused.
                    // - Caller is required to ensure `memory_access` is correct.
                    unsafe {
                        mapper
                            .map(to, from, lock_frames, memory_access)
                            .expect("failed to map range");
                    }
                } else if PageTableInfo::is_large_pages_enabled()
                    && remaining_bytes >= LargeFrame::size_in_bytes()
                    && from.trailing_zeros() >= LargeFrame::size_in_bytes().trailing_zeros()
                    && to.trailing_zeros() >= LargeFrame::size_in_bytes().trailing_zeros()
                {
                    remaining_bytes -= LargeFrame::size_in_bytes();

                    // Map a large page ...

                    let from = LargePage::new(from).expect("non-canonical overrun");
                    let to = LargeFrame::new(to).expect("non-canonical overrun");

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so is unused.
                    // - Caller is required to ensure `memory_access` is correct.
                    unsafe {
                        mapper
                            .map(to, from, lock_frames, memory_access)
                            .expect("failed to map range");
                    }
                } else {
                    remaining_bytes =
                        remaining_bytes.saturating_sub(StandardFrame::size_in_bytes());

                    // Map a standard page ...

                    let from = StandardPage::new(from).expect("non-canonical overrun");
                    let to = StandardFrame::new(to).expect("non-canonical overrun");

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so is unused.
                    // - Caller is required to ensure `memory_access` is correct.
                    unsafe {
                        mapper
                            .map(to, from, lock_frames, memory_access)
                            .expect("failed to map range");
                    }
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
                PageTableInfo::is_large_pages_enabled(),
                LargeFrame::size_in_bytes(),
                PageTableInfo::is_large_pages_enabled(),
                HugeFrame::size_in_bytes()
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
                            unsafe {
                                map_range(
                                    &mut kernel_mapper,
                                    HigherHalfDirectMap::offset(prev_start_address).get(),
                                    prev_start_address,
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
                            kernel_virtual_address + offset,
                            kernel_physical_address + offset,
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
                        HigherHalfDirectMap::frame_to_page::<_, StandardPage>(local_apic_frame)
                            .into(),
                        usize::from(local_apic_frame),
                        StandardFrame::size_in_bytes(),
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
pub unsafe fn zero_frame<F: FrameAddress>(frame: F) {
    let hhdm_address = HigherHalfDirectMap::offset(frame.into());
    let ptr = NonNull::<u8>::with_exposed_provenance(hhdm_address);

    // Safety: Caller is required to maintain safety invariants.
    unsafe {
        NonNull::write_bytes(ptr, 0, F::size_in_bytes());
    }
}
