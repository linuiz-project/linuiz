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
    let mmap_size_bytes = boot_services.memory_map_size() + (size_of::<MemoryDescriptor>() * 2);
    let mmap_buffer =
        memory::allocate_pool(boot_services, MemoryType::LOADER_DATA, mmap_size_bytes);
    let total_memory: usize = match boot_services.memory_map(mmap_buffer.buffer) {
        Ok(completion) => completion.unwrap().1,
        Err(error) => panic!("{:?}", error),
    }
    .map(|descriptor| (descriptor.page_count as usize) * memory::PAGE_SIZE)
    .sum::<usize>();

    if total_memory < MINIMUM_MEMORY {
        panic!(
            "system does not have the minimum required {}MB of RAM.",
            MINIMUM_MEMORY / (1024 * 1024)
        );
    } else {
        info!(
            "Verified minimum system memory: {}MB",
            MINIMUM_MEMORY / (1024 * 1024)
        );
    }

    memory::free_pool(boot_services, mmap_buffer);
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
    // Retrieve a raw allocation pointer & size for the system memory map.
    //
    // Remark:
    //  We can't use `memory::allocate_pool`, because the `system_table.boot_services()` would have its lifetime
    //  used to provide a lifetime to the returned pointer buffer. This is a problem because `system_table` has
    //  to be moved into `ExitBootServices`, which isn't possible if `boot_services()` has had its lifetime used
    //  elsewhere.
    let (mmap_ptr, mmap_alloc_size) = {
        let boot_services = system_table.boot_services();
        let mem_descriptor_size = size_of::<MemoryDescriptor>();
        // Determine the total allocation size of the memory map, in bytes (+2 to cover any extraneous entries created before `ExitBootServices`).
        let mmap_alloc_size = boot_services.memory_map_size() + (2 * mem_descriptor_size);
        let alloc_ptr =
            match boot_services.allocate_pool(MemoryType::LOADER_DATA, mmap_alloc_size) {
                Ok(completion) => completion.unwrap(),
                Err(error) => panic!("{:?}", error),
            };

        (alloc_ptr, mmap_alloc_size)
    };

    info!("Finalizing exit from boot services environment.");
    system_table
        .stdout()
        .reset(false)
        .expect_success("failed to reset standard output");

    // Create the byte buffer to the used for filling in memory descriptors. This buffer, on the call to `ExitBootServices`, provides
    // lifetime information, and so cannot be reinterpreted easily.
    let mmap_buffer = unsafe { &mut *slice_from_raw_parts_mut(mmap_ptr, mmap_alloc_size) };
    // After this point point, the previous system_table and boot_services are no longer valid
    let (runtime_table, mmap_iter) =
        match system_table.exit_boot_services(image_handle, mmap_buffer) {
            Ok(completion) => completion.unwrap(),
            Err(error) => panic!("{:?}", error),
        };

    // Remark: For some reason, this cast itself doesn't result in a valid memory map, even provided
    //  the alignment is correct—so we have to read in the memory descriptors.
    let memory_map = unsafe {
        &mut *slice_from_raw_parts_mut(mmap_ptr as *mut MemoryDescriptor, mmap_iter.len())
    };
    for (index, descriptor) in mmap_iter.enumerate() {
        memory_map[index] = *descriptor;
    }

    // Finally, drop into the kernel.
    let kernel_main: efi_boot::KernelMain = unsafe { transmute(kernel_entry_point) };
    let boot_info = BootInfo::new(memory_map, runtime_table, framebuffer);
    kernel_main(boot_info)
}
