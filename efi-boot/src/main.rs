#![no_std]
#![no_main]
#![feature(unsafe_cell_get_mut, negative_impls)]


#[macro_use]
extern crate log;

use uefi::{Handle, proto::{Protocol, loaded_image::LoadedImage}, table::{Boot, SystemTable}};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[no_mangle]
pub extern "C" fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> ! {
    uefi_services::init(&system_table).expect("failed to initialize UEFI services").unwrap();

    info!("Loaded Gsai UEFI bootloader v{}.", VERSION);
    info!("Configuring kernel environment.");

    let boot_services = system_table.boot_services();
    boot_services.set_watchdog_timer(0, 0, None).expect("failed to clear watchdog timer");
    info!("Reset watchdog timer.");

    let image = unsafe { *(boot_services.handle_protocol(image).expect("failed to load image").unwrap().get() as *mut LoadedImage) };
    info!("Loaded boot image.");

    let device_path = unsafe { *(boot_services.handle_protocol(image.device()).expect("failed to load image device").unwrap().get() as *mut DevicePath) };
    info!("Loaded boot image device path.");
    
    loop {}
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
#[derive(Protocol)]
struct DevicePath {
    device_type: DeviceType,
    sub_type: DeviceSubType,
    length: [u8; 2]
}