#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![feature(const_option)]
#![feature(negative_impls)]
#![feature(core_intrinsics)]
#![feature(unsafe_cell_get_mut)]

#[macro_use]
extern crate log;
extern crate rlibc;

mod elf;
mod protocol_helper;

use core::{
    intrinsics::{wrapping_add, wrapping_mul, wrapping_sub},
    mem::{size_of, transmute},
    ptr::slice_from_raw_parts_mut,
};
use elf::{
    program_header::{ProgramHeader, ProgramHeaderType},
    section_header::{SectionHeader, SectionHeaderType},
    ELFHeader64,
};
use protocol_helper::get_protocol_unwrap;
use uefi::{
    prelude::BootServices,
    proto::{
        loaded_image::{DevicePath, LoadedImage},
        media::{
            file::{Directory, File, FileAttribute, FileMode, RegularFile},
            fs::SimpleFileSystem,
        },
    },
    table::{
        boot::{AllocateType, MemoryDescriptor, MemoryType},
        runtime::ResetType,
        Boot, Runtime, SystemTable,
    },
    Handle, ResultExt, Status,
};
use uefi_macros::entry;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const KERNEL_VADDRESS: usize = 0xFFFFFFFF80000000;
const PAGE_SIZE: usize = 0x1000; // 4096

struct PointerBuffer<'buf> {
    pointer: *mut u8,
    buffer: &'buf mut [u8],
}

#[cfg(debug_assertions)]
fn configure_log_level() {
    use log::{set_max_level, LevelFilter};
    set_max_level(LevelFilter::Debug);
}

#[cfg(not(debug_assertions))]
fn configure_log_level() {
    use log::{set_max_level, LevelFilter};
    set_max_level(LevelFilter::Info);
}

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    configure_log_level();

    let kernel_entry_point = {
        uefi_services::init(&system_table).expect_success("failed to unwrap UEFI services");
        info!("Loaded Gsai UEFI bootloader v{}.", VERSION);
        info!("Configuring bootloader environment.");

        let boot_services = system_table.boot_services();
        info!("Acquired boot services from UEFI firmware.");

        // prepare required environment data
        let image = get_protocol_unwrap::<LoadedImage>(boot_services, image_handle)
            .expect("failed to acquire boot image");
        info!("Acquired boot image from boot services.");
        let device_path = get_protocol_unwrap::<DevicePath>(boot_services, image.device())
            .expect("failed to acquire boot image device path");
        info!("Acquired boot image device path.");
        let file_handle = boot_services
            .locate_device_path::<SimpleFileSystem>(device_path)
            .expect_success("failed to acquire file handle from device path");
        info!("Acquired file handle from device path.");
        let file_system = get_protocol_unwrap::<SimpleFileSystem>(boot_services, file_handle)
            .expect("failed to load file system from file handle");
        info!("Acquired file system protocol from file handle.");
        let root_directory = &mut file_system
            .open_volume()
            .expect_success("failed to open boot file system root directory");
        info!("Loaded boot file system root directory.");

        // load kernel
        let mut kernel_file = acquire_kernel_file(root_directory);
        info!("Acquired kernel image file.");
        let kernel_header = acquire_kernel_header(&mut kernel_file);
        info!("Kernel header read into memory.");
        debug!("{:?}", kernel_header);

        allocate_program_segments(boot_services, &mut kernel_file, &kernel_header);
        info!("Allocated all program segments into memory.");
        allocate_section_segments(boot_services, &mut kernel_file, &kernel_header);
        info!("Allocated all section segments into memory.");

        kernel_header.entry_address()
    };

    loop {}

    let mut runtime_table = safe_exit_boot_services(image_handle, system_table);

    // at this point, the given system_table is invalid
    let result = kernel_transfer(kernel_entry_point);

    unsafe {
        runtime_table
            .runtime_services()
            .reset(ResetType::Shutdown, Status::SUCCESS, None);
    }
}

/* HELPER FUNCTIONS */

fn align_up(value: usize, alignment: usize) -> usize {
    let super_aligned = wrapping_add(value, alignment);
    let force_under_aligned = wrapping_sub(super_aligned, 1);
    wrapping_mul(force_under_aligned / alignment, alignment)
}

fn align_down(value: usize, alignment: usize) -> usize {
    (value / alignment) * alignment
}

/// returns the minimum necessary memory pages to contain the given size in bytes.
fn size_to_pages(size: usize) -> usize {
    if (size % PAGE_SIZE) == 0 {
        size / PAGE_SIZE
    } else {
        (size / PAGE_SIZE) + 1
    }
}

fn alloc_pool_buffer(
    boot_services: &BootServices,
    memory_type: MemoryType,
    size: usize,
) -> PointerBuffer {
    let alloc_pointer = match boot_services.allocate_pool(memory_type, size) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    };
    let alloc_buffer = unsafe { &mut *slice_from_raw_parts_mut(alloc_pointer, size) };

    PointerBuffer {
        pointer: alloc_pointer,
        buffer: alloc_buffer,
    }
}

fn alloc_page_buffer(
    boot_services: &BootServices,
    allocate_type: AllocateType,
    memory_type: MemoryType,
    page_count: usize,
) -> PointerBuffer {
    let alloc_pointer = match boot_services.allocate_pages(allocate_type, memory_type, page_count) {
        Ok(completion) => completion.unwrap() as *mut u8,
        Err(error) => panic!("{:?}", error),
    };
    let alloc_buffer =
        unsafe { &mut *slice_from_raw_parts_mut(alloc_pointer, page_count * PAGE_SIZE) };

    PointerBuffer {
        pointer: alloc_pointer,
        buffer: alloc_buffer,
    }
}

fn free_pool_buffer(boot_services: &BootServices, buffer: PointerBuffer) {
    match boot_services.free_pool(buffer.pointer) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    }
}

fn free_page_buffer(boot_services: &BootServices, buffer: PointerBuffer, count: usize) {
    match boot_services.free_pages(buffer.pointer as u64, count) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    }
}

fn open_file<F: File>(current: &mut F, name: &str) -> RegularFile {
    trace!("Attempting to load file system object: {}", name);
    match current.open(name, FileMode::Read, FileAttribute::READ_ONLY) {
        // this is unsafe due to the possibility of passing an invalid file handle to external code
        Ok(completion) => unsafe { RegularFile::new(completion.expect("failed to find file")) },
        Err(error) => panic!("{:?}", error),
    }
}

fn acquire_kernel_file(root_directory: &mut Directory) -> RegularFile {
    let mut kernel_directory = open_file(root_directory, "EFI");
    let mut gsai_directory = open_file(&mut kernel_directory, "gsai");
    let kernel_file = open_file(&mut gsai_directory, "kernel.elf");
    kernel_directory.close();
    gsai_directory.close();
    kernel_file
}

fn acquire_kernel_header(kernel_file: &mut RegularFile) -> ELFHeader64 {
    // allocate a block large enough to hold the header
    let mut kernel_header_buffer = [0u8; size_of::<ELFHeader64>()];

    // read the file into the buffer
    kernel_file
        .read(&mut kernel_header_buffer)
        .expect_success("failed to read kernel header into memory");
    let kernel_header =
        ELFHeader64::parse(&kernel_header_buffer).expect("failed to parse header from buffer");

    kernel_header
}

fn allocate_program_segments(
    boot_services: &BootServices,
    kernel_file: &mut RegularFile,
    kernel_header: &ELFHeader64,
) {
    let mut program_header_buffer = &mut [0u8; size_of::<ProgramHeader>()];

    for index in 0..kernel_header.program_header_count() {
        let current_program_header_offset = kernel_header.program_header_offset()
            + ((index * kernel_header.program_header_size()) as usize);
        // ensure we properly set the offset in the file to read the correct program header
        kernel_file
            .set_position(current_program_header_offset as u64)
            .expect_success("failed to set position of kernel file");
        kernel_file
            .read(program_header_buffer)
            .expect_success("failed to read program header into memory");

        let program_header = ProgramHeader::parse(program_header_buffer)
            .expect("failed to parse program header from buffer");

        if program_header.ph_type() == ProgramHeaderType::PT_LOAD {
            debug!(
                "Identified program header for loading (offset {}:{}): {:?}",
                index, current_program_header_offset, program_header
            );

            // calculate required variables for correctly loading header into memory
            let aligned_page_address = align_down(program_header.physical_address(), PAGE_SIZE);
            let unaligned_offset =
                wrapping_sub(program_header.physical_address(), aligned_page_address);
            let aligned_program_header_size = program_header.memory_size() + unaligned_offset;
            let pages_count = size_to_pages(aligned_program_header_size);

            debug!(
                "Loading program header: pages {}, aligned addr {}, addr offset {}",
                pages_count, aligned_page_address, unaligned_offset
            );

            // allocate pages for header
            let ph_buffer = alloc_page_buffer(
                boot_services,
                AllocateType::Address(aligned_page_address),
                MemoryType::LOADER_CODE,
                pages_count,
            );

            debug!(
                "Defining program header memory range from: offset {}, end {}",
                unaligned_offset, aligned_program_header_size
            );

            // the program entries are unlikely to be aligned to pages, so we must
            // cut a slice into our current memory buffer, so the program entry can
            // be at the proper memory address.
            let proper_read_range =
                &mut ph_buffer.buffer[unaligned_offset..aligned_program_header_size];

            // offset the kernel file and read the program entry into memory
            kernel_file
                .set_position(program_header.offset() as u64)
                .expect_success("failed to set kernel file position to offset");
            kernel_file
                .read(proper_read_range)
                .expect_success("failed to read program entry from kernel file into memory");

            debug!("Allocated memory pages for program header's entry.");
        }
    }
}

fn allocate_section_segments(
    boot_services: &BootServices,
    kernel_file: &mut RegularFile,
    kernel_header: &ELFHeader64,
) {
    let mut section_header_buffer = &mut [0u8; size_of::<SectionHeader>()];

    for index in 0..kernel_header.section_header_count() {
        // calculate the correct file offset on disk
        let current_section_header_offset = kernel_header.section_header_offset()
            + ((index * kernel_header.section_header_size()) as usize);

        // set position in file and read section header into memory
        kernel_file
            .set_position(current_section_header_offset as u64)
            .expect_success("failed to set kernel file position to section header offset");
        kernel_file
            .read(section_header_buffer)
            .expect_success("failed to read section header from kernel file");
        let section_header = SectionHeader::parse(section_header_buffer)
            .expect("failed to read section header from buffer");

        match section_header.sh_type() {
            SectionHeaderType::SHT_NULL => {}
            _ => {
                debug!(
                    "Identified section header for loading (offset {}:{}): {:?}",
                    index, current_section_header_offset, section_header
                );

                // calculate required variables for correctly loading header into memory
                let aligned_page_address = align_down(section_header.address(), PAGE_SIZE);
                let unaligned_offset = wrapping_sub(section_header.address(), aligned_page_address);
                let aligned_section_header_size = section_header.section_size() + unaligned_offset;
                let pages_count = size_to_pages(aligned_section_header_size);

                debug!(
                    "Loading section header: pages {}, aligned addr {}, addr offset {}",
                    pages_count, aligned_page_address, unaligned_offset
                );

                let sh_buffer = alloc_page_buffer(
                    boot_services,
                    AllocateType::Address(aligned_page_address),
                    MemoryType::LOADER_DATA,
                    pages_count,
                );

                debug!(
                    "Defining section header memory range from: offset {}, end {}",
                    unaligned_offset, aligned_section_header_size
                );

                // the section entries are unlikely to be aligned to pages, so we must
                // cut a slice into our current memory buffer, so the section entry can
                // be at the proper memory address.
                let proper_read_range =
                    &mut sh_buffer.buffer[unaligned_offset..aligned_section_header_size];

                // offset the kernel file and read the section entry into memory
                kernel_file
                    .set_position(section_header.offset() as u64)
                    .expect_success("failed to set kernel file position to section offset");
                kernel_file
                    .read(proper_read_range)
                    .expect_success("failed to read section entry from kernel file into memory");
                    debug!("Allocated memory pages for section header's entry.");
            }
        }
    }
}

fn safe_exit_boot_services(
    image_handle: Handle,
    system_table: SystemTable<Boot>,
) -> SystemTable<Runtime> {
    info!("Preparing to exit boot services environment.");
    let mmap_alloc = {
        let boot_services = system_table.boot_services();
        let mem_descriptor_size = size_of::<MemoryDescriptor>();
        let mmap_alloc_size = boot_services.memory_map_size() + (6 * mem_descriptor_size);
        let alloc_pointer =
            match boot_services.allocate_pool(MemoryType::LOADER_DATA, mmap_alloc_size) {
                Ok(completion) => completion.unwrap(),
                Err(error) => panic!("{:?}", error),
            };

        // we HAVE TO use an unsafe transmutation here, otherwise we run into issues with
        // the system_table/boot_services getting consumed to give lifetime information
        // to the buffer (and thus not being able to be moved into the exit_boot_services call)
        unsafe { &mut *slice_from_raw_parts_mut(alloc_pointer, mmap_alloc_size) }
    };

    info!("Finalizing exit from boot services environment.");

    // clear the output
    system_table
        .stdout()
        .reset(false)
        .expect_success("failed to reset standard output");

    // after this point point, the previous system_table and boot_services are no longer valid
    let (runtime_table, _descriptor_iterator) =
        match system_table.exit_boot_services(image_handle, mmap_alloc) {
            Ok(completion) => completion.unwrap(),
            Err(error) => panic!("{:?}", error),
        };

    runtime_table
}

fn kernel_transfer(kernel_entry_point: usize) -> u32 {
    unsafe {
        type KernelMain = fn() -> u32;
        let kernel_main: KernelMain = transmute(kernel_entry_point);

        // and finally, we enter the kernel
        kernel_main()
    }
}
