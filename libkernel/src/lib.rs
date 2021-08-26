#![no_std]
#![feature(
    asm,
    once_cell,
    raw_ref_op,
    step_trait,
    abi_efiapi,
    abi_x86_interrupt,
    panic_info_message,
    alloc_error_handler,
    const_panic,
    const_mut_refs,
    const_fn_trait_bound,
    exclusive_range_pattern,
    const_btree_new
)]

#[macro_use]
extern crate log;
extern crate alloc;

mod addr;
mod boot_info;
mod rwbitarray;

pub mod acpi;
pub mod bit_switch;
pub mod cell;
pub mod elf;
pub mod instructions;
pub mod io;
pub mod memory;
pub mod registers;
pub mod structures;
pub use addr::*;
pub use boot_info::*;
pub use rwbitarray::*;

pub const SYSTEM_SLICE_SIZE: usize = 0x10000000000;

#[cfg(feature = "panic_handler")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!(
        "KERNEL PANIC (at {}): {}",
        info.location().unwrap(),
        info.message().unwrap()
    );

    crate::instructions::hlt_indefinite()
}

#[cfg(feature = "alloc_error_handler")]
#[alloc_error_handler]
fn alloc_error(error: core::alloc::Layout) -> ! {
    error!("KERNEL ALLOCATOR PANIC: {:?}", error);

    crate::instructions::hlt_indefinite()
}

/// Provides a simple mechanism in which the mask of a u64 can be acquired by bit count.
pub const U64_BIT_MASKS: [u64; 64] = [
    0x1,
    0x3,
    0x7,
    0xF,
    0x1F,
    0x3F,
    0x7F,
    0xFF,
    0x1FF,
    0x3FF,
    0x7FF,
    0xFFF,
    0x1FFF,
    0x3FFF,
    0x7FFF,
    0xFFFF,
    0x1FFFF,
    0x3FFFF,
    0x7FFFF,
    0xFFFFF,
    0x1FFFFF,
    0x3FFFFF,
    0x7FFFFF,
    0xFFFFFF,
    0x1FFFFFF,
    0x3FFFFFF,
    0x7FFFFFF,
    0xFFFFFFF,
    0x1FFFFFFF,
    0x3FFFFFFF,
    0x7FFFFFFF,
    0xFFFFFFFF,
    0x1FFFFFFFF,
    0x3FFFFFFFF,
    0x7FFFFFFFF,
    0xFFFFFFFFF,
    0x1FFFFFFFFF,
    0x3FFFFFFFFF,
    0x7FFFFFFFFF,
    0xFFFFFFFFFF,
    0x1FFFFFFFFFF,
    0x3FFFFFFFFFF,
    0x7FFFFFFFFFF,
    0xFFFFFFFFFFF,
    0x1FFFFFFFFFFF,
    0x3FFFFFFFFFFF,
    0x7FFFFFFFFFFF,
    0xFFFFFFFFFFFF,
    0x1FFFFFFFFFFFF,
    0x3FFFFFFFFFFFF,
    0x7FFFFFFFFFFFF,
    0xFFFFFFFFFFFFF,
    0x1FFFFFFFFFFFFF,
    0x3FFFFFFFFFFFFF,
    0x7FFFFFFFFFFFFF,
    0xFFFFFFFFFFFFFF,
    0x1FFFFFFFFFFFFFF,
    0x3FFFFFFFFFFFFFF,
    0x7FFFFFFFFFFFFFF,
    0xFFFFFFFFFFFFFFF,
    0x1FFFFFFFFFFFFFFF,
    0x3FFFFFFFFFFFFFFF,
    0x7FFFFFFFFFFFFFFF,
    0xFFFFFFFFFFFFFFFF,
];

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    ptr: *mut u8,
    size: Size,
    stride: usize,
}

impl FramebufferInfo {
    pub const fn new(ptr: *mut u8, size: Size, stride: usize) -> Self {
        Self { ptr, size, stride }
    }

    pub const fn size(&self) -> Size {
        self.size
    }

    pub const fn stride(&self) -> usize {
        self.stride
    }

    pub fn addr(&self) -> Address<addr_ty::Physical> {
        Address::<addr_ty::Physical>::new(self.ptr as usize)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Size {
    width: usize,
    height: usize,
}

impl Size {
    pub const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    pub const fn len(&self) -> usize {
        self.width() * self.height()
    }
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

pub const fn align_up(value: usize, alignment: usize) -> usize {
    let alignment_mask = alignment - 1;
    if value & alignment_mask == 0 {
        value
    } else {
        (value | alignment_mask) + 1
    }
}

pub const fn align_up_div(value: usize, alignment: usize) -> usize {
    (value + (alignment - 1)) / alignment
}

pub const fn align_down(value: usize, alignment: usize) -> usize {
    value & !(alignment - 1)
}

pub const fn align_down_div(value: usize, alignment: usize) -> usize {
    align_down(value, alignment) / alignment
}
