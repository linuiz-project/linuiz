mod global_alloc;

mod hhdm;
pub use hhdm::*;

mod stack;
pub use stack::*;

// pub mod io;
pub mod mapper;
pub mod paging;
pub mod pmm;

use crate::{
    interrupts::InterruptCell,
    mem::{
        mapper::Mapper,
        paging::{PageTableEntry, TableDepth, TableEntryFlags},
        pmm::PhysicalMemoryManager,
    },
};
use libsys::{Address, Frame, table_index_size};
use spin::{Mutex, Once};

static KERNEL_MAPPER: Once<InterruptCell<Mutex<Mapper>>> = Once::new();

/// Initialize the kernel memory system. This will:
/// - set up the kernel page table mapper
/// - map & flag each entry from the bootloader memory map
/// - map & flag the kernel executable regions
#[allow(clippy::too_many_lines)]
pub fn init(
    memory_map_request: &limine::request::MemoryMapRequest,
    kernel_file_request: &limine::request::ExecutableFileRequest,
    kernel_address_request: &limine::request::ExecutableAddressRequest,
) {
    KERNEL_MAPPER.call_once(|| {
        debug!("Preparing kernel memory system.");

        let mut kernel_mapper = Mapper::new(TableDepth::max());

        // Prepare the memory map iterator by deconstructing the memory map entry into its requisite parts.
        let mut memory_map_iter = memory_map_request
            .get_response()
            .expect("bootloader did not provide a response to the memory map request")
            .entries()
            .iter()
            .map(|entry| {
                let entry_start = usize::try_from(entry.base).unwrap();
                let entry_end = usize::try_from(entry.base + entry.length).unwrap();

                (entry_start..entry_end, entry.entry_type)
            });

        // Extract the first entry to use as an initialization value for the proceeding `.fold()`.
        let first_entry = memory_map_iter.next().expect("memory map has no entries");
        // Iterate each entry, keeping track of the previous to allow us to map the regions inbetween entries.
        memory_map_iter.fold(first_entry, |(prev_range, prev_ty), (range, ty)| {
            if prev_range.end == range.start && prev_ty == ty {
                return (prev_range.start..range.end, ty);
            } else if range.start > prev_range.end {
                // If there's space inbetween entries, we want to map that as well. Although, the memory
                // will be locked in the physical memory manager, to ensure it isn't accidentally written to.

                map_hhdm_range(
                    &mut kernel_mapper,
                    prev_range.end..range.start,
                    TableEntryFlags::RW,
                    true,
                );
            }

            match ty {
                limine::memory_map::EntryType::USABLE => {
                    map_hhdm_range(
                        &mut kernel_mapper,
                        range.clone(),
                        TableEntryFlags::RW,
                        false,
                    );
                }

                limine::memory_map::EntryType::ACPI_NVS
                | limine::memory_map::EntryType::ACPI_RECLAIMABLE
                | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
                | limine::memory_map::EntryType::FRAMEBUFFER => {
                    map_hhdm_range(&mut kernel_mapper, range.clone(), TableEntryFlags::RW, true);
                }

                limine::memory_map::EntryType::RESERVED
                | limine::memory_map::EntryType::EXECUTABLE_AND_MODULES => {
                    map_hhdm_range(&mut kernel_mapper, range.clone(), TableEntryFlags::RO, true);
                }

                limine::memory_map::EntryType::BAD_MEMORY => {
                    trace!("HHDM Map (!! BAD MEMORY !!) @{range:#X?}");
                }

                _ => unreachable!("unrecognized memory map entry type"),
            }

            (range, ty)
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
                unsafe extern "C" {
                    static KERNEL_BASE: libkernel::LinkerSymbol;
                }

                debug!("{program_header:X?}");

                // Safety: `KERNEL_BASE` is a linker symbol to an in-executable memory location, set by the linker.
                let kernel_base = unsafe { KERNEL_BASE.as_usize() };
                let base_offset = usize::try_from(program_header.p_vaddr).unwrap() - kernel_base;
                let base_offset_end =
                    base_offset + usize::try_from(program_header.p_memsz).unwrap();
                let flags = TableEntryFlags::from(crate::task::segment_to_mmap_permissions(
                    program_header.p_flags,
                ));

                (base_offset..base_offset_end)
                    .step_by(libsys::page_size())
                    .for_each(|offset| {
                        let physical_address =
                            Address::new(kernel_physical_address + offset).unwrap();
                        let virtual_address =
                            Address::new(kernel_virtual_address + offset).unwrap();

                        trace!("Map  {virtual_address:X?} -> {physical_address:X?}   {flags:?}");
                        kernel_mapper
                            .map(
                                virtual_address,
                                TableDepth::min(),
                                physical_address,
                                true,
                                flags,
                            )
                            .expect("failed to map kernel memory region");
                    });
            });

        debug!("Switching to kernel page tables...");

        // Safety: Kernel mappings should be identical to the bootloader mappings.
        unsafe {
            kernel_mapper.swap_into();
        }

        debug!("Kernel has finalized control of page tables.");

        InterruptCell::new(Mutex::new(kernel_mapper))
    });
}

fn map_hhdm_range(
    mapper: &mut crate::mem::mapper::Mapper,
    mut range: core::ops::Range<usize>,
    flags: TableEntryFlags,
    lock_frames: bool,
) {
    let huge_page_depth = TableDepth::new(1).unwrap();

    trace!("HHDM Map  {range:#X?}  {flags:?}   lock: {lock_frames}");

    let frame_address = Address::new(range.start).unwrap();
    let page_address = Hhdm::frame_to_page(frame_address);

    while !range.is_empty() {
        if range.len() > huge_page_depth.align()
            && range.start.trailing_zeros() >= huge_page_depth.align().trailing_zeros()
        {
            // Map a huge page

            range.advance_by(huge_page_depth.align()).unwrap();

            mapper
                .map(
                    page_address,
                    huge_page_depth,
                    frame_address,
                    lock_frames,
                    flags | TableEntryFlags::HUGE,
                )
                .expect("failed to map range");
        } else {
            // Map a standard page

            range.advance_by(libsys::page_size()).unwrap();

            mapper
                .map(
                    page_address,
                    TableDepth::min(),
                    frame_address,
                    lock_frames,
                    flags,
                )
                .expect("failed to map range");
        }
    }
}

pub fn with_kernel_mapper<T>(func: impl FnOnce(&mut Mapper) -> T) -> T {
    KERNEL_MAPPER
        .get()
        .expect("kernel memory has not been initialized")
        .with(|mapper| {
            let mut mapper = mapper.lock();
            func(&mut mapper)
        })
}

pub fn copy_kernel_page_table() -> Result<Address<Frame>, pmm::Error> {
    let table_frame = PhysicalMemoryManager::next_frame()?;
    let table_ptr =
        core::ptr::with_exposed_provenance_mut(Hhdm::offset().get() + table_frame.get().get());

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
        #[cfg(target_arch = "x86_64")]
        crate::arch::x86_64::registers::control::CR3::write(args.0, args.1);

        #[cfg(target_arch = "riscv64")]
        crate::arch::rv64::registers::satp::write(args.0.as_usize(), args.1, args.2);
    }

    #[inline]
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
