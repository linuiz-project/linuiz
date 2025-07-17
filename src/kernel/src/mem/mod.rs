mod hhdm;
pub use hhdm::*;

// pub mod io;
pub mod alloc;
pub mod mapper;
pub mod paging;
pub mod pmm;
pub mod stack;

use crate::{
    interrupts::InterruptCell,
    mem::{
        mapper::Mapper,
        paging::{PageTableEntry, TableDepth, TableEntryFlags},
        pmm::PhysicalMemoryManager,
    },
};
use libsys::{Address, Frame, Page, giga_page_size, mega_page_size, page_size, table_index_size};
use spin::{Mutex, Once};

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
        paging_flags: TableEntryFlags,
    ) {
        trace!("Map Range: ({from:X?} -> {to:X?}):{length:#X} {paging_flags:?}");

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
                    .map(
                        from,
                        TableDepth::giga(),
                        to,
                        false,
                        paging_flags | TableEntryFlags::HUGE,
                    )
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
                    .map(
                        from,
                        TableDepth::mega(),
                        to,
                        false,
                        paging_flags | TableEntryFlags::HUGE,
                    )
                    .expect("failed to map range");

                remaining_length -= mega_page_size();
            } else {
                // Map a standard page

                mapper
                    .map(from, TableDepth::min(), to, false, paging_flags)
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

        let mut kernel_mapper = Mapper::new(TableDepth::max());

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
                let entry_paging_flags = {
                    match entry.entry_type {
                        limine::memory_map::EntryType::USABLE
                        | limine::memory_map::EntryType::ACPI_NVS
                        | limine::memory_map::EntryType::ACPI_RECLAIMABLE
                        | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
                        | limine::memory_map::EntryType::FRAMEBUFFER => TableEntryFlags::RW,

                        limine::memory_map::EntryType::RESERVED
                        | limine::memory_map::EntryType::EXECUTABLE_AND_MODULES => {
                            TableEntryFlags::RO
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
                    entry_paging_flags,
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
                let segment_paging_flags = TableEntryFlags::from(
                    crate::task::segment_to_mmap_permissions(program_header.p_flags),
                );

                map_range(
                    &mut kernel_mapper,
                    segment_page,
                    segment_frame,
                    segment_length,
                    segment_paging_flags,
                );
            });

        // Safety: Kernel page tables should be set up correctly.
        unsafe {
            kernel_mapper.swap_into();
        }

        trace!("Kernel has finalized control of memory system.");

        InterruptCell::new(Mutex::new(kernel_mapper))
    });
}

pub fn with_kernel_mapper<T>(func: impl FnOnce(&mut Mapper) -> T) -> T {
    KERNEL_MAPPER.wait().with(|mapper| {
        let mut mapper = mapper.lock();
        func(&mut mapper)
    })
}

pub fn copy_kernel_page_table() -> Result<Address<Frame>, pmm::Error> {
    let table_frame = PhysicalMemoryManager::next_frame()?;
    let table_ptr = core::ptr::with_exposed_provenance_mut(
        HigherHalfDirectMap::frame_to_page(table_frame).get().get(),
    );

    // Safety: Frame is provided by allocator, and so guaranteed to be within the HHDM, and is frame-sized.
    let new_table = unsafe { core::slice::from_raw_parts_mut(table_ptr, table_index_size()) };
    new_table.fill(PageTableEntry::empty());
    with_kernel_mapper(|kmapper| new_table.copy_from_slice(kmapper.view_page_table()));

    Ok(table_frame)
}

#[cfg(target_arch = "x86_64")]
pub struct PagingRegister(
    pub Address<Frame>,
    pub crate::arch::x86_64::registers::control::CR3Flags,
);
#[cfg(target_arch = "riscv64")]
pub struct PagingRegister(
    pub Address<Frame>,
    pub u16,
    pub crate::arch::rv64::registers::satp::Mode,
);

impl PagingRegister {
    pub fn read() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let args = crate::arch::x86_64::registers::control::CR3::read();
            Self(args.0, args.1)
        }

        #[cfg(target_arch = "riscv64")]
        {
            let args = crate::arch::rv64::registers::satp::read();
            Self(args.0, args.1, args.2)
        }
    }

    /// # Safety
    ///
    /// Writing to this register has the chance to externally invalidate memory references.
    pub unsafe fn write(args: &Self) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::registers::control::CR3::write(args.0, args.1);

            #[cfg(target_arch = "riscv64")]
            crate::arch::rv64::registers::satp::write(args.0.as_usize(), args.1, args.2);
        }
    }

    pub const fn frame(&self) -> Address<Frame> {
        self.0
    }
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
