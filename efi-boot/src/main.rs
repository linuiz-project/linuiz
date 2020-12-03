#![no_std]
#![no_main]

mod efi;

use core::panic::PanicInfo;
use efi::{EFIHandle, EFISystemTable, EFISimpleTextOutputProtocol, EFIStatus};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn efi_main(image: EFIHandle, system_table: EFISystemTable) -> EFIStatus {
    let stdout = system_table.get_console_out();
    stdout.reset(false);
    stdout.write_string("this is a test");

    loop {}

    EFIStatus::Success
}