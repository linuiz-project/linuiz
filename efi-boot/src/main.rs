#![no_std]
#![no_main]
#![feature(unsafe_cell_get_mut, negative_impls, abi_efiapi, const_option)]

#[macro_use]
extern crate log;

mod elf;
mod protocol_helper;

use core::{fmt::Pointer, ptr::slice_from_raw_parts_mut};
use elf::headers::ELFHeader64;
use protocol_helper::get_protocol_unwrap;
use uefi::{
    prelude::BootServices,
    proto::{
        loaded_image::{DevicePath, LoadedImage},
        media::{
            file::{Directory, File, FileAttribute, FileInfo, FileMode, RegularFile},
            fs::SimpleFileSystem,
        },
    },
    table::boot::MemoryDescriptor,
    table::boot::MemoryMapKey,
    table::{boot::MemoryType, Boot, Runtime, SystemTable},
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
    {
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
            kernel_file.set_position(current_program_header_offset as u64);
        }
    }

    exit_and_kernel_transfer(image_handle, system_table)
}

/* HELPER FUNCTIONS */

fn align(size: usize, alignment: usize) -> usize {
    size + ((alignment - (size % alignment)) % alignment)
}

fn alloc_buffer(
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
    kernel_file.read(&mut kernel_header_buffer).ok().unwrap();
    let kernel_header = match ELFHeader64::parse(&kernel_header_buffer) {
        Some(header) => header,
        None => panic!("failed to parse header from buffer"),
    };

    // return the kernel header
    kernel_header
}

fn exit_and_kernel_transfer(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
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
    let (_runtime_table, _descriptor_iterator) =
        match system_table.exit_boot_services(image_handle, mmap_alloc) {
            Ok(completion) => completion.unwrap(),
            Err(error) => panic!("{:?}", error),
        };

    Status::SUCCESS
}

/// returns the minimum necessary memory pages to contain the given size in bytes.
fn size_to_pages(size: usize) -> usize {
    if (size % PAGE_SIZE) == 0 {
        size / PAGE_SIZE
    } else {
        (size / PAGE_SIZE) + 1
    }
}
