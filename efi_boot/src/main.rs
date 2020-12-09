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

use core::{
    mem::{size_of, transmute},
    ptr::slice_from_raw_parts_mut,
};
use efi_boot::drivers::ProtocolGraphics;
use file::open_file;
use protocol::{get_protocol, locate_protocol};
use uefi::{
    prelude::BootServices,
    proto::{
        console::gop::{GraphicsOutput, Mode},
        loaded_image::{DevicePath, LoadedImage},
        media::{
            file::{Directory, File, RegularFile},
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
const MINIMUM_MEMORY: usize = 0xF424000; // 256MB

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
    // we have to encapsulate our usage of system_table.boot_services() so its lifetime doesn't
    // keep us from calling exit_boot_services() later
    let kernel_entry_point = {
        uefi_services::init(&system_table).expect_success("failed to unwrap UEFI services");
        info!("Loaded Gsai UEFI bootloader v{}.", VERSION);

        configure_log_level();
        info!("Configured log level to '{:?}'.", log::max_level());
        info!("Configuring bootloader environment.");

        let boot_services = system_table.boot_services();
        info!("Acquired boot services from UEFI firmware.");

        // test to see how much memory we're working with
        ensure_enough_memory(boot_services);

        // prepare required environment data
        let image = get_protocol::<LoadedImage>(boot_services, image_handle)
            .expect("failed to acquire boot image");
        info!("Acquired boot image from boot services.");
        let device_path = get_protocol::<DevicePath>(boot_services, image.device())
            .expect("failed to acquire boot image device path");
        info!("Acquired boot image device path.");
        let file_handle = boot_services
            .locate_device_path::<SimpleFileSystem>(device_path)
            .expect_success("failed to acquire file handle from device path");
        info!("Acquired file handle from device path.");
        let file_system = get_protocol::<SimpleFileSystem>(boot_services, file_handle)
            .expect("failed to load file system from file handle");
        info!("Acquired file system protocol from file handle.");
        let root_directory = &mut file_system
            .open_volume()
            .expect_success("failed to open boot file system root directory");
        info!("Loaded boot file system root directory.");

        // crate graphics output for kernel
        let graphics_output =
            locate_protocol::<GraphicsOutput>(boot_services).expect("no graphics output!");
        info!("Acquired graphics output protocol.");
        let protocol_graphics = ProtocolGraphics::new(boot_services, graphics_output);

        // load kernel
        let kernel_file = acquire_kernel_file(root_directory);
        info!("Acquired kernel image file.");
        kernel_loader::load_kernel(boot_services, kernel_file)
    };

    let runtime_table = safe_exit_boot_services(image_handle, system_table);

    // at this point, the given system_table is invalid
    type KernelMain = fn(bool) -> u32;
    let kernel_main: KernelMain = unsafe { transmute(kernel_entry_point) };
    //let _result = kernel_main(false);

    unsafe {
        runtime_table
            .runtime_services()
            .reset(ResetType::Shutdown, Status::SUCCESS, None);
    }
}

fn ensure_enough_memory(boot_services: &BootServices) {
    let minimum_address = MINIMUM_MEMORY - memory::PAGE_SIZE;
    if let Ok(completion) = boot_services.allocate_pages(
        AllocateType::Address(minimum_address),
        MemoryType::LOADER_DATA,
        1,
    ) {
        let allocated_address = completion.unwrap();
        boot_services.free_pages(allocated_address, 1);
    } else {
        panic!(
            "Host system requires a minimum of {} of RAM.",
            MINIMUM_MEMORY / (0xF4240/* 1MB */)
        );
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
