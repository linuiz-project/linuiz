#![no_std]
#![no_main]
#![feature(unsafe_cell_get_mut, negative_impls, abi_efiapi, const_option)]

#[macro_use]
extern crate log;

mod protocol_helper;

use core::{fmt::Pointer, ptr::slice_from_raw_parts_mut};
use elf_rs::Elf;
use protocol_helper::get_protocol_unwrap;
use uefi_macros::entry;
use uefi::{
    Handle, Status, 
    prelude::BootServices, 
    proto::{loaded_image::{LoadedImage, DevicePath}, 
    media::{file::{Directory, File, FileAttribute, FileInfo, FileMode, RegularFile}, fs::SimpleFileSystem}}, 
    table::boot::MemoryDescriptor, table::boot::MemoryMapKey, 
    table::{Boot, SystemTable, boot::MemoryType}};


const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const KERNEL_VADDRESS: usize = 0xFFFFFFFF80000000;
const PAGE_SIZE: usize = 4096;

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect("failed to initialize UEFI services").expect("failed to unwrap UEFI services");
    log::info!("Loaded Gsai UEFI bootloader v{}.", VERSION);
    info!("Configuring bootloader environment.");

    let boot_services = system_table.boot_services();
    info!("Acquired boot services from UEFI firmware.");

    // prepare required environment data
    let image = get_protocol_unwrap::<LoadedImage>(boot_services, image_handle).expect("failed to acquire boot image");
    info!("Acquired boot image from boot services.");
    let device_path = get_protocol_unwrap::<DevicePath>(boot_services, image.device()).expect("failed to acquire boot image device path");
    info!("Acquired boot image device path.");
    let file_handle = boot_services.locate_device_path::<SimpleFileSystem>(device_path).expect("failed to acquire file handle from device path").unwrap();
    info!("Acquired file handle from device path.");
    let file_system = get_protocol_unwrap::<SimpleFileSystem>(boot_services, file_handle).expect("failed to load file system from file handle");
    info!("Acquired file system protocol from file handle.");
    let root_directory = &mut file_system.open_volume().expect("failed to open boot file system root directory").unwrap();
    info!("Loaded boot file system root directory.");
    
    // load kernel
    let mut kernel_file = acquire_kernel_file(root_directory);
    info!("Acquired kernel image file.");
    let kernel_memory = read_kernel_into_memory(&boot_services, &mut kernel_file);
    info!("Kernel image data successfully read into memory.");
    let kernel_raw_elf = Elf::from_bytes(kernel_memory.buffer).expect("failed to parse ELF from kernel image");
    info!("ELF file successfully parsed from kernel image.");

    info!("Preparing to exit boot services environment.");
    let mmap_alloc_size = boot_services.memory_map_size() + (2 * core::mem::size_of::<MemoryDescriptor>());
    let mmap_alloc_buffer = unsafe {
        &mut *slice_from_raw_parts_mut(
            match boot_services.allocate_pool(MemoryType::LOADER_DATA, mmap_alloc_size) {
                Ok(completion) => completion.unwrap(),
                Err(error) => panic!("failed to allocate pooled memory for memory map: {:?}", error)
        }, mmap_alloc_size)
    };

    info!("Finalizing exit from boot services environment.");
    let (_runtime_table, _descriptor_iterator) = 
        match system_table.exit_boot_services(image_handle, mmap_alloc_buffer) {
            Ok(completion) => completion.unwrap(),
            Err(error) => panic!("{:?}", error)
        };

    // at this point we can no longer utilize boot services (that includes logging)
    enter_kernel_main(kernel_raw_elf);

    loop {

    }
}


/* HELPER FUNCTIONS */

fn align(size: usize, alignment: usize) -> usize {
    size + ((alignment - (size % alignment)) % alignment)
}

fn open_file<F: File>(current: &mut F, name: &str) -> RegularFile {
    trace!("Attempting to load file system object: {}", name);
    match current.open(name, FileMode::Read, FileAttribute::READ_ONLY) {
        // this is unsafe due to the possibility of passing an invalid file handle to external code
        Ok(completion) => unsafe { RegularFile::new(completion.expect("failed to find file")) },
        Err(error) => panic!("{:?}", error)
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

fn read_kernel_into_memory<'buf>(boot_services: &BootServices, kernel_file: &mut RegularFile) -> PointerBuffer<'buf> {
    let file_info_buffer = &mut [0u8; 256];
    let kernel_file_size = kernel_file.get_info::<FileInfo>(file_info_buffer).unwrap().unwrap().file_size() as usize;
    let allocated_address_ptr = 
        match boot_services.allocate_pool(MemoryType::BOOT_SERVICES_CODE, kernel_file_size) {
            Ok(memory_address) => {
                trace!("Partially allocated kernel image memory, attempting to unwrap...");
                memory_address.unwrap()
            },
            Err(error) => panic!("failed to allocate memory for kernel image: {:?}", error)
        };

    trace!("Unwrapped and fully allocated memory for kernel image.");

    // read the kernel image into pooled memory
    let allocated_buffer = unsafe { &mut *slice_from_raw_parts_mut(allocated_address_ptr, kernel_file_size) };
    match kernel_file.read(allocated_buffer) {
        Ok(completion) => {
            if completion.unwrap() != kernel_file_size {
                panic!("buffer too small to read entire kernel file");
            } else {
                PointerBuffer {
                    pointer: allocated_address_ptr,
                    buffer: allocated_buffer
                }
            }
        },
        Err(_) => panic!("TODO pass required size")
    }
}

/// returns the minimum necessary memory pages to contain the given size in bytes.
fn size_to_pages(size: usize) -> usize {
    if (size % PAGE_SIZE) == 0 {
        size / PAGE_SIZE
    } else {
        (size / PAGE_SIZE) + 1
    }
}

fn enter_kernel_main(kernel_raw_elf: Elf) {
    if let Elf::Elf64(kernel_elf) = kernel_raw_elf {
        unsafe {
            type EntryPoint = fn();
            let entry_point = kernel_elf.header().entry_point() as *const EntryPoint;
            (*entry_point)();
        }
    }
}

struct PointerBuffer<'buf> {
    pointer: *mut u8,
    buffer: &'buf mut [u8]
}