#![no_std]
#![no_main]
#![feature(unsafe_cell_get_mut, negative_impls, abi_efiapi)]

#[macro_use]
extern crate log;


use core::cell::UnsafeCell;
use uefi_macros::entry;
use uefi::{Guid, Handle, Identify, Status, proto::{Protocol, loaded_image::LoadedImage, media::fs::SimpleFileSystem}, prelude::BootServices, table::{Boot, SystemTable}, unsafe_guid};


const VERSION: &'static str = env!("CARGO_PKG_VERSION");

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
    
    let file_system = locate_protocol_unwrap::<SimpleFileSystem>(boot_services).expect("failed to acquire file system.");
    info!("Successfully acquired boot file system.");
    let mut _root_directory = file_system.open_volume().expect("failed to open boot file system root directory").with_status(Status::SUCCESS).unwrap();
    info!("Successfully loaded boot file system root directory.");
    
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

#[repr(u8)]
#[derive(Debug)]
pub enum DeviceType {
    Hardware = 0x01,
    ACPI = 0x02,
    Messaging = 0x03,
    Media = 0x04,
    BIOSBootSpec = 0x05,
    End = 0x7F
}

#[repr(u8)]
#[derive(Debug)]
pub enum DeviceSubType {
    EndInstance = 0x01,
    EndEntire = 0xFF
}

#[repr(C)]
#[unsafe_guid("09576e91-6d3f-11d2-8e39-00a0c969723b")]
pub struct DevicePath {
    pub device_type: DeviceType,
    pub sub_type: DeviceSubType,
    pub length: [u8; 2]
}

impl Protocol for DevicePath {
    
}