#![no_std]
#![feature(
    asm,
    const_fn,
    once_cell,
    abi_efiapi,
    const_panic,
    const_mut_refs,
    core_intrinsics,
    abi_x86_interrupt,
    panic_info_message,
    alloc_error_handler,
    const_raw_ptr_to_usize_cast
)]

#[macro_use]
extern crate log;
extern crate alloc;

mod bitarray;
mod boot_info;

pub mod acpi;
pub mod elf;
pub mod instructions;
pub mod io;
pub mod memory;
pub mod registers;
pub mod structures;
pub use bitarray::*;
pub use boot_info::*;
pub use x86_64::{PhysAddr, VirtAddr};

pub const SYSTEM_SLICE_SIZE: usize = 0x10000000000;

#[cfg(feature = "kernel_impls")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!(
        "KERNEL PANIC (at {}): {}",
        info.location().unwrap(),
        info.message().unwrap()
    );

    loop {}
}

#[cfg(feature = "kernel_impls")]
#[alloc_error_handler]
fn alloc_error(error: core::alloc::Layout) -> ! {
    error!("KERNEL ALLOCATOR PANIC: {:?}", error);

    loop {}
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct FramebufferPointer {
    pub pointer: *mut u8,
    pub size: Size,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Size {
    pub width: usize,
    pub height: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FFIOption<T> {
    None,
    Some(T),
}

impl<T> Into<Option<T>> for FFIOption<T> {
    fn into(self) -> Option<T> {
        match self {
            FFIOption::Some(some) => Some(some),
            FFIOption::None => None,
        }
    }
}

pub type KernelMain<MM, CTE> = extern "efiapi" fn(crate::BootInfo<MM, CTE>) -> !;

pub fn align_up(value: usize, alignment: usize) -> usize {
    assert!(
        alignment.is_power_of_two(),
        "`alignment` must be a power of two"
    );

    let alignment_mask = alignment - 1;
    if value & alignment_mask == 0 {
        value
    } else {
        (value | alignment_mask) + 1
    }
}

const fn align_up_div(value: usize, alignment: usize) -> usize {
    (value + (alignment - 1)) / alignment
}

pub fn align_down(value: usize, alignment: usize) -> usize {
    assert!(
        alignment.is_power_of_two(),
        "alignment must be a power of two"
    );

    value & !(alignment - 1)
}
