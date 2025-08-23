use crate::{
    mem::{mapper::Mapper, mapper::paging::Depth},
    task::asid::AddressSpaceId,
};
use libsys::{
    address::{Address, Frame, Page},
    constants::{huge_page_size, large_page_size, page_size},
};

mod hhdm;
pub use hhdm::*;

// pub mod io;
pub mod alloc;
pub mod mapper;
pub mod pmm;

crate::singleton! {
    #[derive(Debug)]
    pub struct KernelMapper {
        mapper: Mapper
    }

    fn init(
        memory_map_request: &limine::request::MemoryMapRequest,
        kernel_file_request: &limine::request::ExecutableFileRequest,
        kernel_address_request: &limine::request::ExecutableAddressRequest
    ) -> Self {
        fn map_range(
            mapper: &mut Mapper,
            from: Address<Page>,
            to: Address<Frame>,
            count: usize,
            memory_access: Permissions,
        ) {
            let virtual_start = from.get().get();
            let virtual_end = virtual_start + count;
            trace!(
                "Map Range: {:#X?} -> {:#X} {{ {memory_access:?} }}",
                virtual_start..virtual_end,
                to.get().get()
            );

            let mut remaining_count = count;
            while remaining_count > 0 {
                let offset = count - remaining_count;
                let from = Address::<Page>::new(from.get().get() + offset).unwrap();
                let to = Address::<Frame>::new(to.get().get() + offset).unwrap();

                if mapper::use_huge_pages()
                    // check is larger than giga page
                    && remaining_count >= huge_page_size()
                    // check is aligned to giga page
                    && from.get().get().trailing_zeros() >= huge_page_size().trailing_zeros()
                {
                    // Map a giga page

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so
                    //   is unused.
                    // - `memory_access` is calculated based on the type of the
                    //   memory region, as reported by the bootloader (so should
                    //   be correct, if the bootloader is not lying).
                    unsafe {
                        mapper
                            .map(from, to, Depth::giga(), false, memory_access)
                            .expect("failed to map range");
                    }

                    remaining_count -= huge_page_size();
                } else if mapper::use_large_pages()
                    // check is larger than mega page
                    && remaining_count >= large_page_size()
                    // check is aligned to mega page
                    && from.get().get().trailing_zeros() >= large_page_size().trailing_zeros()
                {
                    // Map a mega page

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so
                    //   is unused.
                    // - `memory_access` is calculated based on the type of the
                    //   memory region, as reported by the bootloader (so should
                    //   be correct, if the bootloader is not lying).
                    unsafe {
                        mapper
                            .map(from, to, Depth::mega(), false, memory_access)
                            .expect("failed to map range");
                    }

                    remaining_count -= large_page_size();
                } else {
                    // Map a standard page

                    // Safety:
                    // - `from` page is not mapped in current page tables.
                    // - `to` frame is apart of the higher-half direct map, so
                    //   is unused.
                    // - `memory_access` is calculated based on the type of the
                    //   memory region, as reported by the bootloader (so should
                    //   be correct, if the bootloader is not lying).
                    unsafe {
                        mapper
                            .map(from, to, Depth::max(),false, memory_access)
                            .expect("failed to map range");
                    }

                    remaining_count -= core::cmp::min(page_size(), remaining_count);
                }
            }
        }

        debug!("Preparing kernel memory...");
        debug!(
            "Paging Setup Info: {{ large pages: {}, huge pages: {} }}",
            mapper::use_large_pages(),
            mapper::use_huge_pages(),
        );


        let mut kernel_mapper = Mapper::new();

        trace!("Mapping the higher-half direct map...");
        memory_map_request
            .get_response()
            .expect("bootloader did not provide a response to the memory map request")
            .entries()
            .iter()
            .for_each(|entry| {
                trace!(
                    "Map Entry: {{ start: {:#X}, length: {:#X}, type: {} }}",
                    entry.base,
                    entry.length,
                    crate::limine_memory_map_entry_type_to_str(entry.entry_type)
                );

                let entry_start = usize::try_from(entry.base).unwrap();
                let entry_length = usize::try_from(entry.length).unwrap();
                let entry_frame = Address::<Frame>::new(entry_start).unwrap();
                let entry_page = HigherHalfDirectMap::frame_to_page(entry_frame);
                let entry_permissions = {
                    match entry.entry_type {
                        limine::memory_map::EntryType::USABLE
                        | limine::memory_map::EntryType::ACPI_NVS
                        | limine::memory_map::EntryType::ACPI_RECLAIMABLE
                        | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
                        | limine::memory_map::EntryType::FRAMEBUFFER
                            => Permissions::ReadWrite,

                        limine::memory_map::EntryType::RESERVED
                        | limine::memory_map::EntryType::EXECUTABLE_AND_MODULES
                            => Permissions::ReadOnly,

                        _ => {
                            unreachable!("Unrecognized memory map entry type: {:#X}", entry.base)
                        }
                    }
                };

                map_range(
                    &mut kernel_mapper,
                    entry_page,
                    entry_frame,
                    entry_length,
                    entry_permissions,
                );
            });

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


        trace!("Mapping the kernel executable...");
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
                let segment_page = Address::<Page>::new(kernel_virtual_address + offset).unwrap();
                let segment_frame =
                    Address::<Frame>::new(kernel_physical_address + offset).unwrap();
                let segment_length = usize::try_from(core::cmp::max(
                    program_header.p_memsz, // If the segment size is smaller than it's alignment, we can map it
                    program_header.p_align, // as if it's alignment is the total size (support for mega pages).
                ))
                .unwrap();
                let segment_permissions =
                    crate::task::segment_to_mapping_permissions(program_header.p_flags);

                map_range(
                    &mut kernel_mapper,
                    segment_page,
                    segment_frame,
                    segment_length,
                    segment_permissions,
                );
            });

        #[cfg(target_arch = "x86_64")]
        {
            let local_apic_frame = crate::arch::x86_64::registers::model_specific::IA32_APIC_BASE::get_base_address();

            trace!("Mapping the local APIC: {local_apic_frame:X?}");

            map_range(
                &mut kernel_mapper,
                HigherHalfDirectMap::frame_to_page(local_apic_frame),
                local_apic_frame,
                1,
                Permissions::ReadWrite
            );
        }


        let kernel_mapper = Self {
            mapper: kernel_mapper
        };

        debug!("Kernel mappings complete.");
        trace!("{kernel_mapper:#X?}");

        kernel_mapper
    }
}

impl KernelMapper {
    pub fn clone() -> Mapper {
        Self::get_static().mapper.clone()
    }

    pub unsafe fn swap_into() {
        let mapper = &Self::get_static().mapper;

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

// pub unsafe fn catch_read(ptr: NonNull<[u8]>) -> Result<Box<[u8]>, Exception>
// {     let mem_range = ptr.as_uninit_slice().as_ptr_range();
//     let aligned_start = libsys::align_down(mem_range.start.addr(),
// libsys::page_shift());     let mem_end = mem_range.end.addr();

//     let mut copied_mem = Box::new_uninit_slice(ptr.len());
//     for (offset, page_addr) in
// (aligned_start..mem_end).enumerate().step_by(page_size()) {         let
// ptr_addr = core::cmp::max(mem_range.start.addr(), page_addr);         let
// ptr_len = core::cmp::min(mem_end.saturating_sub(ptr_addr), page_size());

//         // Safety: Box slice and this iterator are bound by the ptr len.
//         let to_ptr = unsafe { copied_mem.as_mut_ptr().add(offset) };
//         // Safety: Copy is only invalid if the caller provided an invalid
// pointer.         crate::local::do_catch(|| unsafe {
//             core::ptr::copy_nonoverlapping(ptr_addr as *mut u8, to_ptr,
// ptr_len);         })?;
//     }

//     Ok(copied_mem)
// }

// TODO TryString
// pub unsafe fn catch_read_str(mut read_ptr: NonNull<u8>) -> Result<String,
// Exception> {     let mut strlen = 0;
//     'y: loop {
//         let read_len = read_ptr.as_ptr().align_offset(page_size());
//         read_ptr = NonNull::new(
//             // Safety: This pointer isn't used without first being validated.
//             unsafe { read_ptr.as_ptr().add(page_size() - read_len) },
//         )
//         .unwrap();

//         for byte in catch_read(NonNull::slice_from_raw_parts(read_ptr,
// read_len))?.iter() {             if byte.ne(&b'\0') {
//                 strlen += 1;
//             } else {
//                 break 'y;
//             }
//         }
//     }

//     Ok(String::from_utf8_lossy(core::slice::from_raw_parts(read_ptr.as_ptr(),
// strlen)).into_owned()) }

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
