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
    exclusive_range_pattern,
    extern_types,
    ptr_as_uninit,
    slice_ptr_get,
    const_align_offset,
    const_transmute_copy,
    const_ptr_as_ref,
    const_option,
    const_slice_from_raw_parts,
    const_ptr_is_null,
    naked_functions
)]

#[macro_use]
extern crate log;
extern crate alloc;

mod addr;
mod macros;

pub mod acpi;
pub mod cell;
pub mod collections;
pub mod cpu;
pub mod elf;
pub mod instructions;
pub mod io;
pub mod memory;
pub mod registers;
pub mod structures;
pub mod sync;
pub use addr::*;

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

    pub fn addr(&self) -> Address<Physical> {
        Address::<Physical>::new(self.ptr as usize)
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

#[repr(C)]
pub struct BootInfo<MM, CTE> {
    memory_map_ptr: *const MM,
    memory_map_len: usize,
    config_table_ptr: *const CTE,
    config_table_len: usize,
    magic: u32,
    framebuffer: FFIOption<FramebufferInfo>,
}

impl<MM, CTE> BootInfo<MM, CTE> {
    const MAGIC: u32 = 0xAABB11FF;

    pub fn new(
        memory_map: &[MM],
        config_table: &[CTE],
        framebuffer: Option<FramebufferInfo>,
    ) -> Self {
        Self {
            memory_map_ptr: memory_map.as_ptr(),
            memory_map_len: memory_map.len(),
            config_table_ptr: config_table.as_ptr(),
            config_table_len: config_table.len(),
            magic: Self::MAGIC,
            framebuffer: match framebuffer {
                Some(some) => FFIOption::Some(some),
                None => FFIOption::None,
            },
        }
    }

    pub fn memory_map(&self) -> &[MM] {
        unsafe {
            core::ptr::slice_from_raw_parts(self.memory_map_ptr, self.memory_map_len)
                .as_ref()
                .unwrap()
        }
    }

    pub fn config_table(&self) -> &'static [CTE] {
        unsafe {
            core::ptr::slice_from_raw_parts(self.config_table_ptr, self.config_table_len)
                .as_ref()
                .unwrap()
        }
    }

    pub fn framebuffer_pointer(&self) -> Option<FramebufferInfo> {
        self.framebuffer.into()
    }

    pub fn validate_magic(&self) {
        assert_eq!(
            self.magic,
            Self::MAGIC,
            "boot_info is unaligned, or magic is otherwise corrupted"
        );
    }
}

pub type KernelMain<MM, CTE> = extern "efiapi" fn(crate::BootInfo<MM, CTE>) -> !;

pub const KIBIBYTE: usize = 0x400; // 1024
pub const MIBIBYTE: usize = KIBIBYTE * KIBIBYTE;
pub const GIBIBYTE: usize = MIBIBYTE * MIBIBYTE;
pub const PT_L4_ENTRY_MEM: u64 = 1 << 9 << 9 << 9 << 12;

#[inline(always)]
pub const fn to_kibibytes(value: usize) -> usize {
    value / KIBIBYTE
}

#[inline(always)]
pub const fn to_mibibytes(value: usize) -> usize {
    value / MIBIBYTE
}

#[inline(always)]
pub const fn align_up(value: usize, alignment: usize) -> usize {
    let alignment_mask = alignment - 1;
    if value & alignment_mask == 0 {
        value
    } else {
        (value | alignment_mask) + 1
    }
}

// TODO use u64 for these alignment functions
#[inline(always)]
pub const fn align_up_div(value: usize, alignment: usize) -> usize {
    ((value + alignment) - 1) / alignment
}

#[inline(always)]
pub const fn align_down(value: usize, alignment: usize) -> usize {
    value & !(alignment - 1)
}

#[inline(always)]
pub const fn align_down_div(value: usize, alignment: usize) -> usize {
    align_down(value, alignment) / alignment
}

extern "C" {
    pub type LinkerSymbol;
}

impl LinkerSymbol {
    #[inline]
    pub const unsafe fn as_ptr<T>(&'static self) -> *const T {
        self as *const _ as *const _
    }

    #[inline]
    pub const unsafe fn as_mut_ptr<T>(&'static self) -> *mut T {
        self as *const _ as *mut _
    }

    #[inline]
    pub unsafe fn as_usize(&'static self) -> usize {
        self as *const _ as usize
    }

    #[inline]
    pub unsafe fn as_u64(&'static self) -> u64 {
        self as *const _ as u64
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
