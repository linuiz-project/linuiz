#![no_std]
#![no_main]

mod efi;

use core::panic::PanicInfo;
use efi::{EFIHandle, EFISystemTable, EFIStatus};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn efi_main(_image: EFIHandle, system_table: EFISystemTable) -> EFIStatus {
    let stdout = system_table.get_console_out();
    stdout.reset(false).ok();
    stdout.print_many(&["Loaded Gsai UEFI bootloader v", VERSION, ".\r\n"]).ok();
    stdout.println("Configuring bootloader environment.");

    loop {}
}