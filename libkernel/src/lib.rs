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
    const_ptr_is_null,
    naked_functions,
    allocator_api,
    sync_unsafe_cell,
    asm_sym,
    asm_const,
    core_intrinsics
)]

extern crate alloc;
extern crate log;

mod addr;
mod macros;

pub use addr::*;
pub mod collections;
pub mod elf;
pub mod io;
pub mod memory;
// TODO decide if I can move the `registers` module to `kernel::`
//pub mod registers;
pub mod sync;
pub mod syscall;

pub enum ReadOnly {}
pub enum WriteOnly {}
pub enum ReadWrite {}

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
        formatter.debug_tuple("Index Ring").field(&format_args!("{}/{}", self.current, self.max - 1)).finish()
    }
}

// /// Generates a random number within the given range, or [Option::None] if [crate::instructions::rdrand64] is unavaible.
// TODO this should be arch-independent
// pub fn rand(range: core::ops::Range<u64>) -> Option<u64> {
//     crate::instructions::rdrand().ok().map(|initial| {
//         let rand_absolute_factor = u64::MAX / initial;
//         let slide = (range.end - range.start) / rand_absolute_factor;
//         range.start + slide
//     })
// }
