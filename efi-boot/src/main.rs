#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![feature(const_option)]
#![feature(negative_impls)]
#![feature(core_intrinsics)]
#![feature(unsafe_cell_get_mut)]

#[macro_use]
extern crate log;

mod elf;
mod protocol_helper;

use core::{
    intrinsics::{wrapping_add, wrapping_mul, wrapping_sub},
    ptr::slice_from_raw_parts_mut,
};
use elf::headers::{ELFHeader64, ProgramHeader, ProgramHeaderType};
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
    Handle, Status,
};
use uefi_macros::entry;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const KERNEL_VADDRESS: usize = 0xFFFFFFFF80000000;
const PAGE_SIZE: usize = 4096;

struct PointerBuffer<'buf> {
    pointer: *mut u8,
    buffer: &'buf mut [u8],
}

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    let kernel_entry_point = {
        uefi_services::init(&system_table)
            .expect("failed to initialize UEFI services")
            .expect("failed to unwrap UEFI services");
        log::info!("Loaded Gsai UEFI bootloader v{}.", VERSION);
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
            .expect("failed to acquire file handle from device path")
            .unwrap();
        info!("Acquired file handle from device path.");
        let file_system = get_protocol_unwrap::<SimpleFileSystem>(boot_services, file_handle)
            .expect("failed to load file system from file handle");
        info!("Acquired file system protocol from file handle.");
        let root_directory = &mut file_system
            .open_volume()
            .expect("failed to open boot file system root directory")
            .unwrap();
        info!("Loaded boot file system root directory.");

        // load kernel
        let mut kernel_file = acquire_kernel_file(root_directory);
        info!("Acquired kernel image file.");
        let kernel_header = acquire_kernel_header(&mut kernel_file);
        info!("Kernel header read into memory.");

        for index in 0..kernel_header.program_header_count() {
            let current_program_header_offset = kernel_header.program_header_offset()
                + ((index * kernel_header.program_header_size()) as usize);
            // ensure we properly set the offset in the file to read the correct program header
            kernel_file
                .set_position(current_program_header_offset as u64)
                .ok()
                .unwrap()
                .unwrap();

            // get program header
            let mut program_header_buffer = [0u8; core::mem::size_of::<ProgramHeader>()];
            kernel_file
                .read(&mut program_header_buffer)
                .ok()
                .unwrap()
                .unwrap();

            let program_header = ProgramHeader::parse(&program_header_buffer)
                .expect("failed to parse program header from buffer");

            if program_header.ph_type() == ProgramHeaderType::PT_LOAD {
                info!(
                    "Identified program header for loading: {:?}",
                    program_header
                );

                // calculate required variables for correctly loading header into memory
                let aligned_page_address = align_down(program_header.physical_address(), PAGE_SIZE);
                let unaligned_offset =
                    wrapping_sub(program_header.physical_address(), aligned_page_address);
                let aligned_program_header_size = program_header.memory_size() + unaligned_offset;
                let pages_count = size_to_pages(aligned_program_header_size);

                info!(
                    "Loading program header: pages {}, aligned addr {}, addr offset {}",
                    pages_count, aligned_page_address, unaligned_offset
                );

                // allocate pages for header
                let ph_buffer = page_alloc_buffer(
                    boot_services,
                    AllocateType::Address(aligned_page_address),
                    MemoryType::LOADER_CODE,
                    pages_count,
                );

                info!(
                    "Defining program header memory range from: offset {}, end {}",
                    unaligned_offset, aligned_program_header_size
                );
                let proper_read_range =
                    &mut ph_buffer.buffer[unaligned_offset..aligned_program_header_size];
                // offse the kernel file to read from the program's file offset
                kernel_file
                    .set_position(program_header.offset() as u64)
                    .ok()
                    .unwrap()
                    .unwrap();

                // read the program into
                kernel_file.read(proper_read_range).ok().unwrap().unwrap();

                info!("Allocated memory pages for program header.");
            }
        }

        kernel_header.entry_address()
    };

    let runtime_table = safe_exit_boot_services(image_handle, system_table);

    kernel_transfer(kernel_entry_point);

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

fn pool_alloc_buffer(
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

fn page_alloc_buffer(
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

fn free_buffer(boot_services: &BootServices, buffer: PointerBuffer) {
    match boot_services.free_pool(buffer.pointer) {
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
    let mut kernel_header_buffer = [0u8; core::mem::size_of::<ELFHeader64>()];

    // read the file into the buffer
    kernel_file
        .read(&mut kernel_header_buffer)
        .ok()
        .unwrap()
        .unwrap();
    let kernel_header = match ELFHeader64::parse(&kernel_header_buffer) {
        Some(header) => header,
        None => panic!("failed to parse header from buffer"),
    };

    // return the kernel header
    kernel_header
}

fn safe_exit_boot_services(
    image_handle: Handle,
    system_table: SystemTable<Boot>,
) -> SystemTable<Runtime> {
    info!("Preparing to exit boot services environment.");
    let mmap_alloc = {
        let boot_services = system_table.boot_services();
        let mem_descriptor_size = core::mem::size_of::<MemoryDescriptor>();
        let mmap_alloc_size = boot_services.memory_map_size() + (6 * mem_descriptor_size);
        let alloc_pointer =
            match boot_services.allocate_pool(MemoryType::LOADER_DATA, mmap_alloc_size) {
                Ok(completion) => completion.unwrap(),
                Err(error) => panic!("{:?}", error),
            };

        unsafe { &mut *slice_from_raw_parts_mut(alloc_pointer, mmap_alloc_size) }
    };

    info!("Finalizing exit from boot services environment.");
    let (runtime_table, _descriptor_iterator) =
        match system_table.exit_boot_services(image_handle, mmap_alloc) {
            Ok(completion) => completion.unwrap(),
            Err(error) => panic!("{:?}", error),
        };

    runtime_table
}

fn kernel_transfer(entry_point_address: usize) {
    unsafe {
        type EntryPoint = extern "C" fn() -> !;
        let entry_point = entry_point_address as *const EntryPoint;
        (*entry_point)();
    }
}
