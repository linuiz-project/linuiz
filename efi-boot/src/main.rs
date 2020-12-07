#![no_std]
#![no_main]
#![feature(unsafe_cell_get_mut, negative_impls, abi_efiapi, const_option)]

#[macro_use]
extern crate log;

mod protocol_helper;

use core::{ptr::slice_from_raw_parts_mut, intrinsics::transmute};
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
const PAGE_SIZE: usize = 4096;
const KERNEL_VADDRESS: usize = 0xFFFFFFFF80000000;

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect("failed to initialize UEFI services").expect("failed to unwrap UEFI services");
    info!("Loaded Gsai UEFI bootloader v{}.", VERSION);
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
    let kernel_memory = allocate_kernel_memory(&boot_services, &mut kernel_file);
    info!("Kernel image data successfully read into memory.");
    let kernel_raw_elf = match Elf::from_bytes(&kernel_memory.buffer) {
        Ok(elf) => {
            info!("ELF file successfully parsed from kernel image.");
            elf
        },
        Err(error) => panic!("Failed to parse ELF from kernel image: {:?}", error)
    };

    // allocate space for memory map
    let memory_map_allocation_size = boot_services.memory_map_size() + /* padding */ (2 * core::mem::size_of::<MemoryDescriptor>());
    let memory_map_pool_allocation_ptr = match boot_services.allocate_pool(MemoryType::LOADER_DATA, memory_map_allocation_size) {
        Ok(completion) => match completion.status() {
            Status::SUCCESS => completion.unwrap(),
            status => panic!("failed to allocate pooled memory for memory map: {:?}", status)
        },
        Err(error) => panic!("failed to allocate pooled memory for memory map: {:?}", error)
    };
    let mut memory_map_allocation_buffer = unsafe {
        &mut *slice_from_raw_parts_mut(memory_map_pool_allocation_ptr, memory_map_allocation_size)
    };

    // get memory map
    // let memory_map = match boot_services.memory_map(memory_map_allocation_buffer) {
    //     Ok(completion) => match completion.status() {
    //         Status::SUCCESS => completion.unwrap(),
    //         status => panic!("failed to read memory map: {:?}", status)
    //     },
    //     Err(error) => panic!("failed to read memory map: {:?}", error)
    // };

    info!("Preparing to exit boot services environment.");
    let (runtime_table, descriptor_iterator) = 
        match system_table.exit_boot_services(image_handle, memory_map_allocation_buffer) {
            Ok(completion) => completion.unwrap(),
            Err(error) => panic!("{:?}", error)
        };
    info!("Exited UEFI boot services environment.");
    warn!("UEFI boot services are no longer usable beyond this point.");

   // enter_kernel_main(kernel_memory.pointer, kernel_raw_elf);




    loop {

    }
}


/* HELPER FUNCTIONS */


fn open_file<F: File>(current: &mut F, name: &str) -> RegularFile {
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

fn allocate_kernel_memory<'buf>(boot_services: &BootServices, kernel_file: &mut RegularFile) -> PointerBuffer<'buf> {
    let file_info_buffer = &mut [0u8; 255];
    let kernel_file_size = kernel_file.get_info::<FileInfo>(file_info_buffer).unwrap().unwrap().file_size() as usize;
    let minimum_pages_count = size_to_pages(kernel_file_size);
    let allocated_address_ptr = 
        match boot_services.allocate_pool(MemoryType::LOADER_CODE, minimum_pages_count) {
            Ok(memory_address) =>
                match memory_address.status() {
                    Status::SUCCESS => memory_address.unwrap() as *mut u8,
                    status => panic!("failed to allocate memory for kernel image: {:?}", status)
                },
            Err(error) => panic!("failed to allocate memory for kernel image: {:?}", error)
        };

    // read the kernel image into pooled memory
    let allocated_buffer = unsafe { &mut *slice_from_raw_parts_mut(allocated_address_ptr, minimum_pages_count * PAGE_SIZE) };
    match kernel_file.read(allocated_buffer) {
        Ok(completion) => {
            let size = completion.unwrap();
            if size != kernel_file_size {
                panic!("failed to correctly read full kernel image (unknown error)");
            } else {
                PointerBuffer {
                    pointer: allocated_address_ptr,
                    buffer: allocated_buffer
                }
            }
        },
        Err(error_option) =>
            match error_option.data() {
                Some(_required_size) => panic!("TODO pass required size"),
                None => panic!("{:?}", error_option.status())
            }
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

fn enter_kernel_main(base_address: *mut u8, kernel_raw_elf: Elf) {
    if let Elf::Elf64(kernel_elf) = kernel_raw_elf {
        info!("Entering kernel...");

        unsafe {
            type EntryPoint = extern "C" fn (i32) -> i32;
            info!("{}", kernel_elf.header().entry_point());
            let entry_point_ptr = base_address.add(kernel_elf.header().entry_point() as usize) as *const ();
            let entry_point: EntryPoint = transmute(entry_point_ptr);
            entry_point(-1);
        }
    }
}

struct PointerBuffer<'buf> {
    pointer: *mut u8,
    buffer: &'buf mut [u8]
}