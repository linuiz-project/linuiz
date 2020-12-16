#![no_std]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

pub mod drivers;
pub mod instructions;
pub mod io;
pub mod structures;

use core::{alloc::Layout, panic::PanicInfo};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[alloc_error_handler]
fn alloc_error(_error: Layout) -> ! {
    loop {}
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeLevel {
    /// Kernel
    Ring0,
    /// Kernel drivers
    Ring1,
    /// User drivers
    Ring2,
    /// User mode
    Ring3
}

impl Into<u8> for PrivilegeLevel {
    fn into(self) -> u8 {
        match self {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3
        }
    }
}

impl From<u8> for PrivilegeLevel {
    fn from(val: u8) -> Self {
        PrivilegeLevel::from(val as u16)
    }
}

impl From<u16> for PrivilegeLevel {
    fn from(val: u16) -> Self {
        match val {
            0 => PrivilegeLevel::Ring0,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::Ring3,
            _ => panic!("invalid privilege level!")
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtAddr(usize);

impl VirtAddr {
    pub const fn zero() -> Self {
        VirtAddr::from(0x0)
    }
}

impl From<u64> for VirtAddr {
    fn from(val: u64) -> Self {
        Self {
            0: val
        }
    }
}