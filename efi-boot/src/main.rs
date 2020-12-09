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
    section_header::SectionHeader,
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
    let kernel_entry_point = {
        uefi_services::init(&system_table).expect_success("failed to unwrap UEFI services");
        info!("Loaded Gsai UEFI bootloader v{}.", VERSION);

        configure_log_level();
        info!("Configured log level to '{:?}'.", log::max_level());
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

        allocate_segments(boot_services, &mut kernel_file, &kernel_header);
        info!("Allocated all program segments into memory.");
        // allocate_sections(boot_services, &mut kernel_file, &kernel_header);
        // info!("Allocated all section segments into memory.");

        kernel_header.entry_address()
    };

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
fn aligned_slices(size: usize, alignment: usize) -> usize {
    ((size + alignment) - 1) / alignment
}

fn allocate_pool(
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

fn allocate_pages(
    boot_services: &BootServices,
    allocate_type: AllocateType,
    memory_type: MemoryType,
    pages_count: usize,
) -> PointerBuffer {
    let alloc_pointer = match boot_services.allocate_pages(allocate_type, memory_type, pages_count)
    {
        Ok(completion) => completion.unwrap() as *mut u8,
        Err(error) => panic!("{:?}", error),
    };
    let alloc_buffer =
        unsafe { &mut *slice_from_raw_parts_mut(alloc_pointer, pages_count * PAGE_SIZE) };

    PointerBuffer {
        pointer: alloc_pointer,
        buffer: alloc_buffer,
    }
}

fn free_pool(boot_services: &BootServices, buffer: PointerBuffer) {
    match boot_services.free_pool(buffer.pointer) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    }
}

fn free_pages(boot_services: &BootServices, buffer: PointerBuffer, count: usize) {
    match boot_services.free_pages(buffer.pointer as u64, count) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    }
}

fn open_file<F: File>(file: &mut F, name: &str) -> RegularFile {
    debug!("Attempting to load file system object: {}", name);
    match file.open(name, FileMode::Read, FileAttribute::READ_ONLY) {
        // this is unsafe due to the possibility of passing an invalid file handle to external code
        Ok(completion) => unsafe { RegularFile::new(completion.expect("failed to find file")) },
        Err(error) => panic!("{:?}", error),
    }
}

fn read_file(file: &mut RegularFile, position: u64, buffer: &mut [u8]) {
    debug!("Reading file contents into memory (pos {}).", position);
    file.set_position(position)
        .expect_success("failed to set position of file");
    file.read(buffer)
        .expect_success("failed to read file into memory");
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

fn allocate_segments(
    boot_services: &BootServices,
    kernel_file: &mut RegularFile,
    kernel_header: &ELFHeader64,
) {
    let mut program_header_buffer = &mut [0u8; size_of::<ProgramHeader>()];
    let mut current_disk_offset = kernel_header.program_headers_offset();

    for index in 0..kernel_header.program_header_count() {
        read_file(
            kernel_file,
            current_disk_offset as u64,
            program_header_buffer,
        );
        let program_header = ProgramHeader::parse(program_header_buffer)
            .expect("failed to parse program header from buffer");

        // ensure we are able to increment disk offset at end of for
        if program_header.ph_type() == ProgramHeaderType::PT_LOAD {
            debug!(
                "Identified program header for loading (index {}, disk offset {}): {:?}",
                index, current_disk_offset, program_header
            );

            // calculate required variables for correctly loading segment into memory
            let aligned_address = align_down(
                program_header.physical_address(),
                program_header.alignment(),
            );
            let relative_address = wrapping_sub(program_header.physical_address(), aligned_address);
            let aligned_size = program_header.memory_size() + relative_address;
            let pages_count = aligned_slices(aligned_size, program_header.alignment());

            debug!(
                "Loading program header: size p{}:mem{}:ua{}, aligned addr {}, unaligned addr {}, addr offset {}",
                pages_count, program_header.memory_size(), aligned_size, aligned_address, program_header.physical_address(), relative_address
            );

            // allocate pages for header
            let ph_buffer = allocate_pages(
                boot_services,
                AllocateType::Address(aligned_address),
                MemoryType::LOADER_CODE,
                pages_count,
            );

            debug!(
                "Defining program header memory range from: offset {}, end {}",
                relative_address, aligned_size
            );

            // the program entries are unlikely to be aligned to pages, so we must
            // cut a slice into our current memory buffer, so the program entry can
            // be at the proper memory address.
            let proper_read_range = &mut ph_buffer.buffer[relative_address..aligned_size];
            read_file(
                kernel_file,
                program_header.offset() as u64,
                proper_read_range,
            );
            debug!("Allocated memory pages for program header's entry.");
        }

        current_disk_offset += kernel_header.program_header_size() as usize;
    }
}

fn determine_section_bounds(
    boot_services: &BootServices,
    kernel_file: &mut RegularFile,
    kernel_header: &ELFHeader64,
) -> (Option<usize>, Option<usize>) {
    // REMARK (TODO?): it seems inefficient to read the section headers twice.
    //      the overhead is probably neglible, but it's something to keep in mind.

    // prepare variables
    let mut low_address: Option<usize> = None;
    let mut high_address: Option<usize> = None;
    let mut section_header_buffer = &mut [0u8; size_of::<SectionHeader>()];

    let mut section_disk_offset = kernel_header.section_headers_offset();
    for index in 0..kernel_header.section_header_count() {
        // set position in file and read section header into memory
        read_file(
            kernel_file,
            section_disk_offset as u64,
            section_header_buffer,
        );
        let section_header = SectionHeader::parse(section_header_buffer)
            .expect("failed to read section header from buffer");

        debug!(
            "Determining address space of section (index {}): {:?}",
            index, section_header
        );

        // use exclusive if to ensure we are able to increment disk offset at end of for
        if section_header.address() > 0x0 {
            // high address is the highest possible address the section overlaps
            let section_high_address = section_header.address() + section_header.entry_size();
            debug!(
                "Determining section address space (index {}): low {}, high {}",
                index,
                section_header.address(),
                section_high_address
            );

            if low_address.is_none() || section_header.address() < low_address.unwrap() {
                low_address = Some(section_header.address());
            }

            if high_address.is_none() || section_high_address > high_address.unwrap() {
                high_address = Some(section_high_address);
            }
        }

        section_disk_offset += kernel_header.section_header_size() as usize;
    }

    (low_address, high_address)
}

fn allocate_sections(
    boot_services: &BootServices,
    kernel_file: &mut RegularFile,
    kernel_header: &ELFHeader64,
) {
    // this will help determine where and how many pages we need to allocate for section entries
    let (low_address_option, high_address_option) =
        determine_section_bounds(boot_services, kernel_file, kernel_header);

    if low_address_option.is_none() || high_address_option.is_none() {
        debug!(
            "Address space for section entires is invalid: low {:?}, high {:?}",
            low_address_option, high_address_option
        );
        return;
    }

    let low_address = low_address_option.unwrap();
    let high_address = high_address_option.unwrap();
    let section_buffer_size = high_address - low_address;
    debug!(
        "Determined section entry address space: low {}, high {}, span {}",
        low_address, high_address, section_buffer_size
    );

    if section_buffer_size == 0x0 {
        return; // no section entries to load
    }

    // get data relative to a low address that is aligned on page boundries
    let aligned_low_address = align_down(low_address, PAGE_SIZE);
    let aligned_section_buffer_size = high_address - aligned_low_address;
    // this offset tells us how far from index 0 we need to travel to get the true bottom of
    // the addressed section memory
    let aligned_section_buffer_offset = low_address - aligned_low_address;
    let pages_count = aligned_slices(aligned_section_buffer_size, PAGE_SIZE);

    debug!(
        "Allocating {} pages at address {} for section buffer.",
        pages_count, aligned_low_address
    );
    // allocate buffer for section entries
    let section_buffer = allocate_pages(
        boot_services,
        AllocateType::Address(aligned_low_address),
        MemoryType::LOADER_DATA,
        pages_count,
    )
    // we just want the buffer, we won't explicitly deallocate this
    .buffer;

    // just a container to hold current section header
    let mut section_header_buffer = &mut [0u8; size_of::<SectionHeader>()];
    let mut section_disk_offset = kernel_header.section_headers_offset();
    for index in 0..kernel_header.section_header_count() {
        read_file(
            kernel_file,
            section_disk_offset as u64,
            section_header_buffer,
        );
        let section_header = SectionHeader::parse(section_header_buffer)
            .expect("failed to read section header from buffer");

        // use exclusive if to ensure we are able to increment disk offset at end of for
        if section_header.entry_size() > 0 {
            debug!(
                "Identified section header for loading (index {}, disk offset {}): {:?}",
                index, section_disk_offset, section_header
            );

            // low address of the section relative to the allocated section buffer
            let relative_section_low_address = section_header.address() - aligned_low_address;
            let relative_section_high_address =
                relative_section_low_address + section_header.entry_size();
            // get slice of buffer representing section
            let section_slice =
                &mut section_buffer[relative_section_low_address..relative_section_high_address];

            read_file(kernel_file, section_header.offset() as u64, section_slice);
            debug!("Allocated memory pages for section header's entry.");
        }

        section_disk_offset += section_header.size();
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
    system_table.boot_services().stall(3_000_000);

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
