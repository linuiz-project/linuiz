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
mod file;
mod kernel_loader;
mod memory;
mod protocol;

use crate::{
    file::open_file,
    protocol::{get_protocol, locate_protocol},
};
use core::{
    mem::{size_of, transmute},
    ptr::slice_from_raw_parts_mut,
};
use efi_boot::{BootInfo, FramebufferPointer};
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
        Boot, SystemTable,
    },
    Handle, ResultExt, Status,
};
use uefi_macros::entry;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const MINIMUM_MEMORY: usize = 0xF424000; // 256MB

#[cfg(debug_assertions)]
fn configure_log_level() {
    log::set_max_level(log::LevelFilter::Debug);
}

#[cfg(not(debug_assertions))]
fn configure_log_level() {
    log::set_max_level(log::LevelFilter::Info);
}

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("failed to unwrap UEFI services");
    info!("Loaded Gsai UEFI bootloader v{}.", VERSION);

    configure_log_level();
    info!("Configured log level to '{:?}'.", log::max_level());
    info!("Configuring bootloader environment.");

    // this ugly little hack is to sever the boot_services' lifetime from the system_table, allowing us
    // to later move the system_table into `kernel_transfer()`
    let boot_services = unsafe { &*(system_table.boot_services() as *const BootServices) };
    info!("Acquired boot services from UEFI firmware.");

    // test to see how much memory we're working with
    ensure_enough_memory(boot_services);

    // acquire graphics output to ensure a gout device
    let framebuffer = match locate_protocol::<GraphicsOutput>(boot_services) {
        Some(graphics_output) => {
            let pointer = graphics_output.frame_buffer().as_mut_ptr() as *mut u8;
            let mode = select_graphics_mode(graphics_output);
            let mode_info = mode.info();
            info!("Selected graphics mode: {:?}", mode_info);
            let resolution = mode_info.resolution();
            let size = efi_boot::Size {
                width: resolution.0,
                height: resolution.1,
            };
            info!("Acquired and configured graphics output protocol.");

            Some(FramebufferPointer { pointer, size })
        }
        None => {
            warn!("No graphics output found. Kernel will default to using serial output.");
            None
        }
    };

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

    // load kernel
    let kernel_file = acquire_kernel_file(root_directory);
    info!("Acquired kernel image file.");
    let kernel_entry_point = kernel_loader::load_kernel(boot_services, kernel_file);

    kernel_transfer(image_handle, system_table, kernel_entry_point, framebuffer)
}

fn ensure_enough_memory(boot_services: &BootServices) {
    let minimum_address = MINIMUM_MEMORY - memory::PAGE_SIZE;
    let allocated_address = boot_services
        .allocate_pages(
            AllocateType::Address(minimum_address),
            MemoryType::LOADER_DATA,
            1,
        )
        .expect_success("host system does not meet minimum memory requirements");

    boot_services
        .free_pages(allocated_address, 1)
        .expect_success("failed to free memory when ensuring host system capacity");

    info!(
        "Verified minimum system memory: {}MB",
        minimum_address / 0xF4240
    );
}

fn select_graphics_mode(graphics_output: &mut GraphicsOutput) -> Mode {
    let graphics_mode = graphics_output
        .modes()
        .map(|mode| mode.expect("warning encountered while querying mode"))
        .last()
        .unwrap();

    graphics_output
        .set_mode(&graphics_mode)
        .expect_success("failed to set graphics mode");

    graphics_mode
}

fn acquire_kernel_file(root_directory: &mut Directory) -> RegularFile {
    let mut kernel_directory = open_file(root_directory, "EFI");
    let mut gsai_directory = open_file(&mut kernel_directory, "gsai");
    let kernel_file = open_file(&mut gsai_directory, "kernel.elf");
    kernel_directory.close();
    gsai_directory.close();
    kernel_file
}

fn kernel_transfer(
    image_handle: Handle,
    system_table: SystemTable<Boot>,
    kernel_entry_point: usize,
    framebuffer: Option<FramebufferPointer>,
) -> Status {
    info!("Preparing to exit boot services environment.");
    let mmap_buffer = {
        let boot_services = system_table.boot_services();
        let mem_descriptor_size = size_of::<MemoryDescriptor>();
        let mmap_alloc_size = boot_services.memory_map_size() + (6 * mem_descriptor_size);
        let alloc_pointer =
            match boot_services.allocate_pool(MemoryType::BOOT_SERVICES_DATA, mmap_alloc_size) {
                Ok(completion) => completion.unwrap(),
                Err(error) => panic!("{:?}", error),
            };

        // we HAVE TO use an unsafe transmutation for this retval, otherwise we run into issues with
        // the system_table/boot_services getting consumed to give lifetime information
        // to the buffer (and thus not being able to be moved into the exit_boot_services call)
        unsafe { &mut *slice_from_raw_parts_mut(alloc_pointer, mmap_alloc_size) }
    };

    info!(
        "Finalizing exit from boot services environment:\n Entrypoint: {}\n Framebuffer: {:?}",
        kernel_entry_point, framebuffer
    );
    // reset the output
    system_table
        .stdout()
        .reset(false)
        .expect_success("failed to reset standard output");

    // after this point point, the previous system_table and boot_services are no longer valid
    let (runtime_table, mmap_iter) =
        match system_table.exit_boot_services(image_handle, mmap_buffer) {
            Ok(completion) => completion.unwrap(),
            Err(error) => panic!("{:?}", error),
        };

    // construct final boot info
    let boot_info = BootInfo {
        mmap_iter: &mmap_iter,
        runtime_table,
        framebuffer,
    };

    // at this point, the given SystemTable<Boot> is invalid, and replaced with the runtime_table (SystemTable<Runtime>)
    let kernel_main: efi_boot::KernelMain = unsafe { transmute(kernel_entry_point) };
    kernel_main(boot_info)
}
