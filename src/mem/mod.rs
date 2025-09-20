use crate::{
    mem::{
        addr::{
            phys::{FrameAddress, HugeFrame, LargeFrame, StandardFrame},
            virt::{HugePage, LargePage, PageAddress, StandardPage},
        },
        mapper::{Mapper, paging::PagingInfo},
    },
    util::{elf::segment_to_mapping_permissions, sync::Once},
};
use core::{num::NonZero, ptr::NonNull};

pub mod addr;
pub mod alloc;
pub mod mapper;
pub mod pmm;
// pub mod physical_map;

mod address_space;
pub use address_space::*;

mod hhdm;
pub use hhdm::*;

// pub mod io;

#[cfg(test)]
pub fn get_paging_depth() -> NonZero<u32> {
    // Safety: Value is non-zero.
    unsafe { NonZero::new_unchecked(4) }
}

#[cfg(not(test))]
pub fn get_paging_depth() -> NonZero<u32> {
    use crate::util::sync::Lazy;

    static PAGING_DEPTH: Lazy<NonZero<u32>> = Lazy::new(|| {
        cfg_select! {
            target_arch = "x86_64" => {
                use crate::arch::x86_64::registers::control::cr4;

                if cr4::CR4::read().contains(cr4::Flags::LA57) {
                    // Safety: Value is non-zero.
                    unsafe{NonZero::new_unchecked(5)}
                } else {
                    // Safety: Value is non-zero.
                    unsafe{NonZero::new_unchecked(4)}
                }
            }
        }
    });

    *PAGING_DEPTH
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
            debug_assert_eq!((from & StandardFrame::NON_INDEX_BIT_MASK.get()), 0);
            debug_assert_eq!((to & StandardFrame::NON_INDEX_BIT_MASK.get()), 0);

            debug!(
                "Map Range: {{ From: {from:#X}, To: {to:#X}, Byte Count: {byte_count:#X}, Access: {memory_access:?} }}"
            );

            let mut remaining_bytes = byte_count;
            while remaining_bytes > 0 {
                let offset = byte_count - remaining_bytes;
                let from = from + offset;
                let to = to + offset;

                trace!("Map Range: {{ Offset: {offset:#X}, Remaining: {remaining_bytes:#X} }}");

                if PagingInfo::is_huge_pages_enabled()
                    && remaining_bytes >= HugeFrame::SIZE_IN_BYTES.get()
                    && from.trailing_zeros() >= HugeFrame::SIZE_IN_BYTES.get().trailing_zeros()
                    && to.trailing_zeros() >= HugeFrame::SIZE_IN_BYTES.get().trailing_zeros()
                {
                    // Map a huge page ...

                    let from = HugePage::new(from).expect("non-canonical overrun");
                    let to = HugeFrame::new(to).expect("non-canonical overrun");

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so is unused.
                    // - Caller is required to ensure `memory_access` is correct.
                    unsafe {
                        mapper
                            .map(to, from, lock_frames, memory_access, false)
                            .expect("failed to map range");
                    }

                    remaining_bytes -= HugeFrame::SIZE_IN_BYTES.get();
                } else if PagingInfo::is_large_pages_enabled()
                    && remaining_bytes >= LargeFrame::SIZE_IN_BYTES.get()
                    && from.trailing_zeros() >= LargeFrame::SIZE_IN_BYTES.get().trailing_zeros()
                    && to.trailing_zeros() >= LargeFrame::SIZE_IN_BYTES.get().trailing_zeros()
                {
                    // Map a large page ...

                    let from = LargePage::new(from).expect("non-canonical overrun");
                    let to = LargeFrame::new(to).expect("non-canonical overrun");

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so is unused.
                    // - Caller is required to ensure `memory_access` is correct.
                    unsafe {
                        mapper
                            .map(to, from, lock_frames, memory_access, false)
                            .expect("failed to map range");
                    }

                    remaining_bytes -= LargeFrame::SIZE_IN_BYTES.get();
                } else {
                    // Map a standard page ...

                    let from = StandardPage::new(from).expect("non-canonical overrun");
                    let to = StandardFrame::new(to).expect("non-canonical overrun");

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so is unused.
                    // - Caller is required to ensure `memory_access` is correct.
                    unsafe {
                        mapper
                            .map(to, from, lock_frames, memory_access, false)
                            .expect("failed to map range");
                    }

                    remaining_bytes =
                        remaining_bytes.saturating_sub(StandardFrame::SIZE_IN_BYTES.get());
                }
            }
        }

        KERNEL_MAPPER.call_once(|| {
            info!("Preparing kernel memory...");
            info!("{PagingInfo:#X?}");

            let mut kernel_mapper = Mapper::new();

            trace!("Mapping the higher-half direct map...");
            let mut mmap_entries = memory_map_request
                .get_response()
                .expect("bootloader did not provide a response to the memory map request")
                .entries()
                .iter()
                .map(|entry| {
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
                .peekable();

            while let Some((start_address, mut length, permissions)) = mmap_entries.next() {
                loop {
                    let end_address = start_address + length;
                    if let Some((next_start_address, next_length, next_permissions)) =
                        mmap_entries.peek().copied()
                        && next_start_address == end_address
                        && next_permissions == permissions
                    {
                        trace!(
                            "Coalescing: {:#X?} <=> {:#X?}",
                            start_address..end_address,
                            end_address..(end_address + next_length)
                        );

                        length += next_length;

                        match mmap_entries.advance_by(1) {
                            Ok(()) => continue,
                            Err(_) => break,
                        }
                    }

                    break;
                }

                unsafe {
                    map_range(
                        &mut kernel_mapper,
                        HigherHalfDirectMap::offset(start_address).get(),
                        start_address,
                        length,
                        permissions,
                        false,
                    );
                }
            }

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
                let local_apic_page =
                    HigherHalfDirectMap::frame_to_page::<_, StandardPage>(local_apic_frame);

                trace!("Mapping the local APIC: {local_apic_frame:X?} -> {local_apic_page:X?}");

                unsafe {
                    kernel_mapper
                        .map(
                            local_apic_frame,
                            local_apic_page,
                            false,
                            Permissions::ReadWrite,
                            false,
                        )
                        .unwrap();
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
pub unsafe fn clear_frame_memory<F: FrameAddress>(frame: F) {
    let hhdm_address = HigherHalfDirectMap::offset(frame.into());
    let ptr = NonNull::<u8>::with_exposed_provenance(hhdm_address);

    // Safety: Caller is required to maintain safety invariants.
    unsafe {
        NonNull::write_bytes(ptr, 0, F::SIZE_IN_BYTES.get());
    }
}
