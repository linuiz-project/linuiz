#![no_std]
#![feature(asm)]
#![feature(const_panic)]
#![feature(const_mut_refs)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(const_raw_ptr_to_usize_cast)]

mod privilege_level;

pub mod drivers;
pub mod instructions;
pub mod io;
pub mod structures;
pub use privilege_level::PrivilegeLevel;

use core::{alloc::Layout, ffi::c_void, panic::PanicInfo};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial!("{}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error(error: Layout) -> ! {
    serial!("{:?}", error);
    loop {}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Address {
    Physical(usize),
    Virtual(usize),
}

impl Address {
    pub const fn as_ptr(self) -> *const c_void {
        match self {
            Address::Virtual(address) => address as *const c_void,
            Address::Physical(address) => address as *const c_void,
        }
    }

    pub const fn as_usize(self) -> usize {
        match self {
            Address::Virtual(address) => address,
            Address::Physical(address) => address,
        }
    }
}
