#![no_std]
#![no_main]

mod efi;

use core::panic::PanicInfo;
use efi::{EFIHandle, EFISystemTable, EFISimpleOutProtocol, EFIStatus};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn efi_main(image: EFIHandle, system_table: EFISystemTable) -> EFIStatus {
    let stdout: &mut EFISimpleOutProtocol = unsafe { &mut *(system_table.console_out) };
    let string = "hello world".as_bytes();
    let mut buf = [0u16; 32];

    for i in 0..string.len() {
        buf[i] = string[i] as u16;
    }

    unsafe {
        (stdout.reset)(stdout, false);
        (stdout.output_string)(stdout, buf.as_ptr());
    }

    loop {}

    EFIStatus::SUCCESS
}