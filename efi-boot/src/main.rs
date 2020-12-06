#![no_std]
#![no_main]
#![feature(unsafe_cell_get_mut, negative_impls, abi_efiapi, const_option)]

#[macro_use]
extern crate log;


use core::cell::UnsafeCell;
use uefi_macros::entry;
use uefi::{table::Boot, CStr16, Handle, Status, prelude::BootServices, proto::{Protocol, media::fs::SimpleFileSystem, loaded_image::{LoadedImage, DevicePath}}, table::SystemTable};


const VERSION: &'static str = env!("CARGO_PKG_VERSION");
static ELF_ROOT: &'static CStr16 = CStr16::from_u16_with_nul(&[b'E'.into(), b'L'.into(), b'F'.into(), b'\0'.into()]).ok().unwrap();

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect("failed to initialize UEFI services").expect("failed to unwrap UEFI services");
    info!("Loaded Gsai UEFI bootloader v{}.", VERSION);
    info!("Configuring bootloader environment.");

    info!("Attempting to acquire boot services from UEFI firmware.");
    let boot_services = system_table.boot_services();
    info!("Successfully acquired boot services from UEFI firmware.");

    info!("Attempting to acquire boot image from boot services.");
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
    
    let mut buffer: [u8; 255] = [0; 255];

    loop {
        let newdir_result = root_directory.read_entry(&mut buffer);
        
        if (newdir_result.is_err()) {
            break
        }

        let newdir_option = newdir_result.ok().unwrap().unwrap();

        if (newdir_option.is_none()) {
            break
        }

        let newdir = newdir_option.unwrap();
        let file_name = newdir.file_name();

        if (file_name == ELF_ROOT) {
            info!("success");
        }

        info!("{:?}", file_name);
    }



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