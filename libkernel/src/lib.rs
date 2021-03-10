#![no_std]
#![feature(
    asm,
    const_fn,
    once_cell,
    raw_ref_op,
    step_trait,
    step_trait_ext,
    abi_efiapi,
    abi_x86_interrupt,
    panic_info_message,
    alloc_error_handler,
    const_panic,
    const_mut_refs,
    const_raw_ptr_to_usize_cast
)]

#[macro_use]
extern crate log;
extern crate alloc;

mod addr;
mod boot_info;
mod rwbitarray;
mod volatile_cell;

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
pub use volatile_cell::*;

pub const SYSTEM_SLICE_SIZE: usize = 0x10000000000;

#[cfg(feature = "kernel_impls")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!(
        "KERNEL PANIC (at {}): {}",
        info.location().unwrap(),
        info.message().unwrap()
    );

    crate::instructions::hlt_indefinite()
}

#[cfg(feature = "kernel_impls")]
#[alloc_error_handler]
fn alloc_error(error: core::alloc::Layout) -> ! {
    error!("KERNEL ALLOCATOR PANIC: {:?}", error);

    crate::instructions::hlt_indefinite()
}

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

    pub const fn addr(&self) -> Address<addr_ty::Physical> {
        Address::<addr_ty::Physical>::new(unsafe { self.ptr as usize })
    }

    pub const fn size(&self) -> Size {
        self.size
    }

    pub const fn stride(&self) -> usize {
        self.stride
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
