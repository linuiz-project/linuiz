#![no_std]
#![no_main]
#![feature(unsafe_cell_get_mut, negative_impls, abi_efiapi, const_option)]

#[macro_use]
extern crate log;


use core::cell::UnsafeCell;
use uefi_macros::entry;
use uefi::{CStr16, Handle, Status, prelude::BootServices, proto::{Protocol, media::fs::SimpleFileSystem, loaded_image::{LoadedImage, DevicePath}}, proto::media::file::Directory, proto::media::file::File, proto::media::file::FileAttribute, proto::media::file::FileInfo, proto::media::file::FileMode, proto::media::file::RegularFile, table::Boot, proto::media::file::FileProtocolInfo, table::SystemTable};


const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const FAIL_ROOT_READ: &'static str = "failed to read root directory";

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect("failed to initialize UEFI services").expect("failed to unwrap UEFI services");
    info!("Loaded Gsai UEFI bootloader v{}.", VERSION);
    info!("Configuring bootloader environment.");

    let boot_services = system_table.boot_services();
    info!("Successfully acquired boot services from UEFI firmware.");

    let image = get_protocol_unwrap::<LoadedImage>(boot_services, image_handle).expect("failed to acquire boot image");
    info!("Successfully acquired boot image from boot services.");
    let device_path = get_protocol_unwrap::<DevicePath>(boot_services, image.device()).expect("failed to acquire boot image device path");
    info!("Successfully acquired boot image device path.");
    let file_handle = boot_services.locate_device_path::<SimpleFileSystem>(device_path).expect("failed to acquire file handle from device path").unwrap();
    info!("Successfully acquired file handle from device path.");
    let file_system = get_protocol_unwrap::<SimpleFileSystem>(boot_services, file_handle).expect("failed to load file system from file handle");
    info!("Successfully acquired file system from file handle.");
    let mut root_directory = file_system.open_volume().expect("failed to open boot file system root directory").unwrap();
    info!("Successfully loaded boot file system root directory.");
    
    let kernel_file = acquire_kernel_file(&mut root_directory);
    info!("Successfully acquired kernel image file.");

    

    loop {}
}

fn get_protocol_unwrap<P: Protocol>(boot_services: &BootServices, handle: Handle) -> Option<&mut P> {
    acquire_protocol_unwrapped(boot_services.handle_protocol(handle))
}

fn locate_protocol_unwrap<P: Protocol>(boot_services: &BootServices) -> Option<&mut P> {
    acquire_protocol_unwrapped(boot_services.locate_protocol::<P>())
}

fn acquire_protocol_unwrapped<P: Protocol>(result: uefi::Result<&UnsafeCell<P>, >) -> Option<&mut P> {
    match result {
        Ok(unsafe_cell_completion) =>{
            info!("Protocol found, attempting to acquire...");

            if !unsafe_cell_completion.status().is_success() {
                panic!("failed to locate and acquire protocol: {:?}", unsafe_cell_completion.status());
            } else {
                info!("Protocol acquired, attempting to unwrap...");
                Some(unsafe { &mut *(unsafe_cell_completion.unwrap().get() as *mut P) })
            }       
        },
        Err(error) => panic!("{:?}", error.status())
    }
}

fn open_file<F: File>(current: &mut F, name: &str) -> RegularFile {
    match current.open(name, FileMode::Read, FileAttribute::READ_ONLY) {
        // this is unsafe due to the possibility of passing an invalid file handle to external code
        Ok(completion) => unsafe { RegularFile::new(completion.expect("failed to find file")) },
        Err(error) => panic!("{:?}", error)
    }
}

fn print_file_name<F: File>(file: &mut F, buffer: &mut [u8]) {
    info!("{}", file.get_info::<FileInfo>(buffer).unwrap().unwrap().file_name());
}

fn acquire_kernel_file<F: File>(root_directory: &mut F) -> RegularFile {
    let kernel_directory = &mut open_file(root_directory, "EFI");
    let gsai_directory = &mut open_file(kernel_directory, "gsai");
    open_file(gsai_directory, "kernel.elf")
}

fn read_directory_entry<'buf>(directory: &mut Directory, read_buffer: &'buf mut [u8]) -> Result<&'buf mut FileInfo, usize> {
    match directory.read_entry(read_buffer) {
        Ok(completion) => {
            let option = completion.expect(FAIL_ROOT_READ);
            
            match option {
                Some(info) => Ok(info),
                None => panic!(FAIL_ROOT_READ)
            }
        },
        Err(error) => match error.data() {
            Some(size) => Err(size.clone()),
            None => panic!("{} {:?}", FAIL_ROOT_READ, error.status())
        }
    }
}