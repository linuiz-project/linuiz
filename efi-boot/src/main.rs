#![no_std]
#![no_main]
#![feature(unsafe_cell_get_mut, negative_impls, abi_efiapi, const_option)]

#[macro_use]
extern crate log;

mod protocol_helper;

use core::ptr::slice_from_raw_parts_mut;
use elf_rs::Elf;
use protocol_helper::get_protocol_unwrap;
use uefi_macros::entry;
use uefi::{Handle, Status,
     proto::{
        media::{fs::SimpleFileSystem, 
            file::{File, FileInfo, FileMode, FileAttribute, RegularFile}}, 
        loaded_image::{LoadedImage, DevicePath}}, 
    table::{Boot, SystemTable, 
        boot::{AllocateType, MemoryType}}
};


const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const PAGE_SIZE: &'static usize = &4096;

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect("failed to initialize UEFI services").expect("failed to unwrap UEFI services");
    info!("Loaded Gsai UEFI bootloader v{}.", VERSION);
    info!("Configuring bootloader environment.");

    let boot_services = system_table.boot_services();
    info!("Acquired boot services from UEFI firmware.");

    let image = get_protocol_unwrap::<LoadedImage>(boot_services, image_handle).expect("failed to acquire boot image");
    info!("Acquired boot image from boot services.");
    let device_path = get_protocol_unwrap::<DevicePath>(boot_services, image.device()).expect("failed to acquire boot image device path");
    info!("Acquired boot image device path.");
    let file_handle = boot_services.locate_device_path::<SimpleFileSystem>(device_path).expect("failed to acquire file handle from device path").unwrap();
    info!("Acquired file handle from device path.");
    let file_system = get_protocol_unwrap::<SimpleFileSystem>(boot_services, file_handle).expect("failed to load file system from file handle");
    info!("Acquired file system protocol from file handle.");
    let mut root_directory = file_system.open_volume().expect("failed to open boot file system root directory").unwrap();
    info!("Loaded boot file system root directory.");
    
    let mut kernel_file = acquire_kernel_file(&mut root_directory);
    info!("Acquired kernel image file.");

    let file_info_buffer = &mut [0u8; 255];
    let kernel_file_info = kernel_file.get_info::<FileInfo>(file_info_buffer).unwrap().unwrap();
    let kernel_file_size = kernel_file_info.file_size() as usize;
    let minimum_pages_count = size_to_pages(kernel_file_size);
    let allocated_address = match boot_services.allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, minimum_pages_count) {
        Ok(memory_address) => match memory_address.status() {
            Status::SUCCESS => memory_address.unwrap(),
            status => panic!("failed to allocate memory for kernel image: {:?}", status)
        },
        Err(error) => panic!("failed to allocate memory for kernel image: {:?}", error)
    };

    let allocated_buffer = unsafe { &mut *slice_from_raw_parts_mut(allocated_address as *mut u8, minimum_pages_count * PAGE_SIZE) };
    let read_bytes = match kernel_file.read(allocated_buffer) {
        Ok(completion) => {
            let size = completion.unwrap();
            if size != kernel_file_size {
                panic!("failed to correctly read full kernel image (unknown error)");
            } else {
                info!("Kernel image data successfully read into memory.");
                size
            }
        },
        Err(error_option) => match error_option.data() {
            Some(_required_size) => panic!("TODO pass required size"),
            None => panic!("{:?}", error_option.status())
        }
    };

    let kernel_elf = match Elf::from_bytes(&allocated_buffer[0..kernel_file_size]) {
        Ok(elf) => {
            info!("ELF file successfully parsed from kernel image.");
            elf
        },
        Err(error) => panic!("Failed to parse ELF from kernel image: {:?}", error)
    };

    if let Elf::Elf64(kernel_elf64) = kernel_elf {
        info!("{:?} header: {:?}", kernel_elf64, kernel_elf64.header());
    }

    loop {}
}

fn open_file<F: File>(current: &mut F, name: &str) -> RegularFile {
    match current.open(name, FileMode::Read, FileAttribute::READ_ONLY) {
        // this is unsafe due to the possibility of passing an invalid file handle to external code
        Ok(completion) => unsafe { RegularFile::new(completion.expect("failed to find file")) },
        Err(error) => panic!("{:?}", error)
    }
}

fn acquire_kernel_file<F: File>(root_directory: &mut F) -> RegularFile {
    let kernel_directory = &mut open_file(root_directory, "EFI");
    let gsai_directory = &mut open_file(kernel_directory, "gsai");
    open_file(gsai_directory, "kernel.elf")
}

/// returns the minimum necessary memory pages to contain the given size in bytes.
fn size_to_pages(size: usize) -> usize {
    if (size % PAGE_SIZE) == 0 {
        size / PAGE_SIZE
    } else {
        (size / PAGE_SIZE) + 1
    }
}