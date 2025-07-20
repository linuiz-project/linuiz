use crate::{
    interrupts::InterruptCell,
    mem::{mapper::Mapper, paging::page_table::Depth},
};
use libsys::{Address, Frame, Page, giga_page_size, mega_page_size, page_size};
use spin::{Mutex, Once};

mod hhdm;
pub use hhdm::*;

// pub mod io;
pub mod alloc;
pub mod mapper;
pub mod paging;
pub mod pmm;
pub mod stack;

static KERNEL_MAPPER: Once<InterruptCell<Mutex<Mapper>>> = Once::new();

/// Initialize the kernel memory. This will:
/// - set up the kernel page table mapper
/// - map & flag each entry from the bootloader memory map
/// - map & flag the kernel executable regions
#[allow(clippy::too_many_lines)]
pub fn init(
    memory_map_request: &limine::request::MemoryMapRequest,
    kernel_file_request: &limine::request::ExecutableFileRequest,
    kernel_address_request: &limine::request::ExecutableAddressRequest,
) {
    fn map_range(
        mapper: &mut Mapper,
        from: Address<Page>,
        to: Address<Frame>,
        length: usize,
        permissions: Permissions,
    ) {
        trace!("Map Range: ({from:X?} -> {to:X?}):{length:#X} {permissions:?}");

        let mut remaining_length = length;
        while remaining_length > 0 {
            let offset = length - remaining_length;
            let from = Address::<Page>::new(from.get().get() + offset).unwrap();
            let to = Address::<Frame>::new(to.get().get() + offset).unwrap();

            if paging::use_giga_pages()
                    // check is larger than giga page
                    && remaining_length >= giga_page_size()
                    // check is aligned to giga page
                    && from.get().get().trailing_zeros() >= giga_page_size().trailing_zeros()
            {
                // Map a giga page

                mapper
                    .map(from, Depth::giga(), to, false, permissions)
                    .expect("failed to map range");

                remaining_length -= giga_page_size();
            } else if paging::use_mega_pages()
                    // check is larger than mega page
                    && remaining_length >= mega_page_size()
                    // check is aligned to mega page
                    && from.get().get().trailing_zeros() >= mega_page_size().trailing_zeros()
            {
                // Map a mega page

                mapper
                    .map(from, Depth::mega(), to, false, permissions)
                    .expect("failed to map range");

                remaining_length -= mega_page_size();
            } else {
                // Map a standard page

                mapper
                    .map(from, Depth::max(), to, false, permissions)
                    .expect("failed to map range");

                remaining_length -= core::cmp::min(page_size(), remaining_length);
            }
        }
    }

    KERNEL_MAPPER.call_once(|| {
        debug!("Preparing kernel memory...");
        debug!(
            "Paging Setup Info: MEGA:{}, GIGA:{}",
            paging::use_mega_pages(),
            paging::use_giga_pages()
        );

        let mut kernel_mapper = Mapper::new();

        memory_map_request
            .get_response()
            .expect("bootloader did not provide a response to the memory map request")
            .entries()
            .iter()
            .for_each(|entry| {
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
                        | limine::memory_map::EntryType::FRAMEBUFFER => Permissions::ReadWrite,

                        limine::memory_map::EntryType::RESERVED
                        | limine::memory_map::EntryType::EXECUTABLE_AND_MODULES => {
                            Permissions::ReadOnly
                        }

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

        // Iterate each segment of the kernel executable file, and memory map it with the proper flags.
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
                let segment_page = Address::new(kernel_virtual_address + offset).unwrap();
                let segment_frame = Address::new(kernel_physical_address + offset).unwrap();
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

        InterruptCell::new(Mutex::new(kernel_mapper))
    });

    KERNEL_MAPPER.wait().with(|kernel_mapper| {
        let kernel_mapper = kernel_mapper.lock();

        // Safety: Kernel page tables should be set up correctly.
        unsafe {
            kernel_mapper.swap_into();
        }

        trace!("Kernel has finalized control of memory system.");
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permissions {
    None,
    ReadOnly,
    ReadWrite,
    ReadExecute,
}

pub fn with_kernel_mapper<T>(func: impl FnOnce(&mut Mapper) -> T) -> T {
    KERNEL_MAPPER.wait().with(|mapper| {
        let mut mapper = mapper.lock();
        func(&mut mapper)
    })
}

// pub unsafe fn catch_read(ptr: NonNull<[u8]>) -> Result<Box<[u8]>, Exception> {
//     let mem_range = ptr.as_uninit_slice().as_ptr_range();
//     let aligned_start = libsys::align_down(mem_range.start.addr(), libsys::page_shift());
//     let mem_end = mem_range.end.addr();

//     let mut copied_mem = Box::new_uninit_slice(ptr.len());
//     for (offset, page_addr) in (aligned_start..mem_end).enumerate().step_by(page_size()) {
//         let ptr_addr = core::cmp::max(mem_range.start.addr(), page_addr);
//         let ptr_len = core::cmp::min(mem_end.saturating_sub(ptr_addr), page_size());

//         // Safety: Box slice and this iterator are bound by the ptr len.
//         let to_ptr = unsafe { copied_mem.as_mut_ptr().add(offset) };
//         // Safety: Copy is only invalid if the caller provided an invalid pointer.
//         crate::local::do_catch(|| unsafe {
//             core::ptr::copy_nonoverlapping(ptr_addr as *mut u8, to_ptr, ptr_len);
//         })?;
//     }

//     Ok(copied_mem)
// }

// TODO TryString
// pub unsafe fn catch_read_str(mut read_ptr: NonNull<u8>) -> Result<String, Exception> {
//     let mut strlen = 0;
//     'y: loop {
//         let read_len = read_ptr.as_ptr().align_offset(page_size());
//         read_ptr = NonNull::new(
//             // Safety: This pointer isn't used without first being validated.
//             unsafe { read_ptr.as_ptr().add(page_size() - read_len) },
//         )
//         .unwrap();

//         for byte in catch_read(NonNull::slice_from_raw_parts(read_ptr, read_len))?.iter() {
//             if byte.ne(&b'\0') {
//                 strlen += 1;
//             } else {
//                 break 'y;
//             }
//         }
//     }

//     Ok(String::from_utf8_lossy(core::slice::from_raw_parts(read_ptr.as_ptr(), strlen)).into_owned())
// }

/// Zeros the higher-half direct mapped memory of `frame`.
///
/// # Safety
///
/// - The higher-half direct mapped memory of `frame` must not be otherwise aliased.
pub unsafe fn zero_frame(frame: Address<Frame>) {
    let page = HigherHalfDirectMap::frame_to_page(frame);
    let ptr = page.as_ptr();

    // Safety: Caller is required to maintain safety invariants.
    unsafe {
        core::ptr::write_bytes(ptr, 0, page_size());
    }
}
