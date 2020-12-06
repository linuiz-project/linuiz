#![no_std]
#![no_main]
#![feature(unsafe_cell_get_mut, negative_impls)]

#[macro_use]
extern crate log;


use uefi::{alloc::Allocator, Guid, Handle, Identify, Status, prelude::BootServices, proto::{Protocol, loaded_image::LoadedImage, media::fs::SimpleFileSystem}, table::{Boot, SystemTable}, unsafe_guid};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[no_mangle]
pub extern "C" fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> ! {
    uefi_services::init(&system_table).expect("failed to initialize UEFI services").expect("failed to unwrap UEFI services");
    info!("Loaded Gsai UEFI bootloader v{}.", VERSION);
    info!("Configuring bootloader environment.");

    info!("Attempting to acquire boot services from UEFI firmware.");
    let boot_services = system_table.boot_services();
    info!("Successfully acquired boot services from UEFI firmware.");

    info!("Attempting to acquire boot image from boot services.");
    let image = get_protocol::<LoadedImage>(boot_services, image_handle).expect("failed to acquire boot image");
    info!("Successfully acquired boot image from boot services.");


    let _device_path = get_protocol::<DevicePath>(boot_services, image.device()).expect("failed to acquire boot image device path");
    info!("Successfully acquired boot image device path.");
    
    let mut file_system = locate_protocol::<SimpleFileSystem>(boot_services).expect("failed to acquire file system.");
    info!("Successfully acquired boot file system.");
    let mut _root_directory = file_system.open_volume().expect("failed to open boot file system root directory").with_status(Status::SUCCESS).unwrap();
    info!("Successfully loaded boot file system root directory.");

    loop {}
}

fn get_protocol<P>(boot_services: &BootServices, handle: Handle) -> Option<&mut P> where P : Protocol 
{
    unsafe {
        match boot_services.handle_protocol(handle) {
            Ok(unsafe_cell_completion) => {
                info!("Protocol found, attempting to acquire...");

                if (!unsafe_cell_completion.status().is_success()) {
                    panic!("failed to acquire protocol: {:?}", unsafe_cell_completion.status());
                } else {
                    info!("Protocol acquired, attempting to unwrap...");
                    Some(&mut *(unsafe_cell_completion.unwrap().get() as *mut P))
                }                
            },
            Err(error) => { error!("{:?}", error.status()); panic!("{:?}", error.status()) }
        }
    }
}

fn locate_protocol<P>(boot_services: &BootServices) -> Option<&mut P> where P : Protocol {
    unsafe {
        match boot_services.locate_protocol::<P>() {
            Ok(unsafe_cell_completion) =>{
                info!("Protocol found, attempting to acquire...");

                if (!unsafe_cell_completion.status().is_success()) {
                    panic!("failed to locate and acquire protocol: {:?}", unsafe_cell_completion.status());
                } else {
                    info!("Protocol acquired, attempting to unwrap...");
                    Some(&mut *(unsafe_cell_completion.unwrap().get() as *mut P))
                }       
            },
            Err(error) => panic!("{:?}", error.status())
        }
    }
}

#[repr(u8)]
enum DeviceType {
    Hardware = 0x01,
    ACPI = 0x02,
    Messaging = 0x03,
    Media = 0x04,
    BIOSBootSpec = 0x05,
    End = 0x7F
}

#[repr(u8)]
enum DeviceSubType {
    EndInstance = 0x01,
    EndEntire = 0xFF
}

#[repr(C)]
#[unsafe_guid("09576e91-6d3f-11d2-8e39-00a0c969723b")]
pub struct DevicePath {
    device_type: DeviceType,
    sub_type: DeviceSubType,
    length: [u8; 2]
}

impl Protocol for DevicePath {
    
}