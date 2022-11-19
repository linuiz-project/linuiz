#![no_std]
#![feature(
    extern_types,                       // #43467 <https://github.com/rust-lang/rust/issues/43467>
    step_trait,                         // #42168 <https://github.com/rust-lang/rust/issues/42168> [POSSIBLY REMOVE]
    strict_provenance,                  // #95228 <https://github.com/rust-lang/rust/issues/95228>
    pointer_is_aligned,                 // #96284 <https://github.com/rust-lang/rust/issues/96284>
    const_option_ext,
)]

mod addr;
mod macros;

use core::num::NonZeroUsize;

pub use addr::*;
pub mod mem;

pub struct ReadOnly;
pub struct WriteOnly;
pub struct ReadWrite;

pub const KIBIBYTE: u64 = 0x400; // 1024
pub const MIBIBYTE: u64 = KIBIBYTE * KIBIBYTE;
pub const GIBIBYTE: u64 = MIBIBYTE * MIBIBYTE;
pub const PT_L4_ENTRY_MEM: u64 = 1 << 9 << 9 << 9 << 12;

#[inline]
pub const fn to_kibibytes(value: u64) -> u64 {
    value / KIBIBYTE
}

#[inline]
pub const fn to_mibibytes(value: u64) -> u64 {
    value / MIBIBYTE
}

#[inline]
pub const fn align_up(value: usize, alignment: NonZeroUsize) -> usize {
    let alignment_mask = alignment.get() - 1;
    if value & alignment_mask == 0 {
        value
    } else {
        (value | alignment_mask) + 1
    }
}

#[inline]
pub const fn align_up_div(value: usize, alignment: NonZeroUsize) -> usize {
    ((value + alignment.get()) - 1) / alignment.get()
}

#[inline]
pub const fn align_down(value: usize, alignment: NonZeroUsize) -> usize {
    value & !(alignment.get() - 1)
}

#[inline]
pub const fn align_down_div(value: usize, alignment: NonZeroUsize) -> usize {
    align_down(value, alignment) / alignment.get()
}

extern "C" {
    pub type LinkerSymbol;
}

impl LinkerSymbol {
    #[inline]
    pub fn as_ptr<T>(&'static self) -> *const T {
        self as *const _ as *const T
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
