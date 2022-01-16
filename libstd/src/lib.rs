#![no_std]
#![feature(
    once_cell,
    raw_ref_op,
    step_trait,
    abi_efiapi,
    abi_x86_interrupt,
    panic_info_message,
    alloc_error_handler,
    const_mut_refs,
    const_ptr_offset,
    const_fn_trait_bound,
    exclusive_range_pattern,
    extern_types,
    ptr_as_uninit,
    slice_ptr_get,
    const_align_offset,
    const_transmute_copy,
    const_ptr_as_ref,
    const_option,
    const_slice_from_raw_parts,
    const_ptr_is_null
)]

#[macro_use]
extern crate log;
extern crate alloc;

mod addr;
mod boot_info;
mod macros;

pub mod acpi;
pub mod cell;
pub mod collections;
pub mod elf;
pub mod instructions;
pub mod io;
pub mod memory;
pub mod registers;
pub mod structures;

pub use addr::*;
pub use boot_info::*;

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
    (1 << 1) - 1,
    (1 << 2) - 1,
    (1 << 3) - 1,
    (1 << 4) - 1,
    (1 << 5) - 1,
    (1 << 6) - 1,
    (1 << 7) - 1,
    (1 << 8) - 1,
    (1 << 9) - 1,
    (1 << 10) - 1,
    (1 << 11) - 1,
    (1 << 12) - 1,
    (1 << 13) - 1,
    (1 << 14) - 1,
    (1 << 15) - 1,
    (1 << 16) - 1,
    (1 << 17) - 1,
    (1 << 18) - 1,
    (1 << 19) - 1,
    (1 << 20) - 1,
    (1 << 21) - 1,
    (1 << 22) - 1,
    (1 << 23) - 1,
    (1 << 24) - 1,
    (1 << 25) - 1,
    (1 << 26) - 1,
    (1 << 27) - 1,
    (1 << 28) - 1,
    (1 << 29) - 1,
    (1 << 30) - 1,
    (1 << 31) - 1,
    (1 << 32) - 1,
    (1 << 33) - 1,
    (1 << 34) - 1,
    (1 << 35) - 1,
    (1 << 36) - 1,
    (1 << 37) - 1,
    (1 << 38) - 1,
    (1 << 39) - 1,
    (1 << 40) - 1,
    (1 << 41) - 1,
    (1 << 42) - 1,
    (1 << 43) - 1,
    (1 << 44) - 1,
    (1 << 45) - 1,
    (1 << 46) - 1,
    (1 << 47) - 1,
    (1 << 48) - 1,
    (1 << 49) - 1,
    (1 << 50) - 1,
    (1 << 51) - 1,
    (1 << 52) - 1,
    (1 << 53) - 1,
    (1 << 54) - 1,
    (1 << 55) - 1,
    (1 << 56) - 1,
    (1 << 57) - 1,
    (1 << 58) - 1,
    (1 << 59) - 1,
    (1 << 60) - 1,
    (1 << 61) - 1,
    (1 << 62) - 1,
    (1 << 63) - 1,
    u64::MAX,
];

pub enum ReadOnly {}
pub enum ReadWrite {}

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
    ((value + alignment) - 1) / alignment
}

pub const fn align_down(value: usize, alignment: usize) -> usize {
    value & !(alignment - 1)
}

pub const fn align_down_div(value: usize, alignment: usize) -> usize {
    align_down(value, alignment) / alignment
}

extern "C" {
    pub type LinkerSymbol;
}

impl LinkerSymbol {
    pub const fn as_ptr<T>(&'static self) -> *const T {
        self as *const _ as *const _
    }

    pub const fn as_mut_ptr<T>(&'static self) -> *mut T {
        self as *const _ as *mut _
    }

    pub fn as_usize(&'static self) -> usize {
        self as *const _ as usize
    }

    pub fn as_page(&'static self) -> memory::Page {
        memory::Page::from_ptr(self.as_ptr::<core::ffi::c_void>())
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptDeliveryMode {
    Fixed = 0b000,
    LowPriority = 0b001,
    SMI = 0b010,
    NMI = 0b100,
    INIT = 0b101,
    StartUp = 0b110,
    ExtINT = 0b111,
}

pub struct IndexRing {
    current: usize,
    max: usize,
}

impl IndexRing {
    pub fn new(max: usize) -> Self {
        Self { current: 0, max }
    }

    pub fn index(&self) -> usize {
        self.current
    }

    pub fn increment(&mut self) {
        self.current = self.next_index();
    }

    pub fn next_index(&self) -> usize {
        (self.current + 1) % self.max
    }
}

impl core::fmt::Debug for IndexRing {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Index Ring")
            .field(&format_args!("{}/{}", self.current, self.max - 1))
            .finish()
    }
}


