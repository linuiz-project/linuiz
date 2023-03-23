#![no_std]
#![feature(
    strict_provenance,          // #95228 <https://github.com/rust-lang/rust/issues/95228>
    const_option,
    const_bool_to_option,
)]

mod macros;

mod address;
use core::num::NonZeroU32;

pub use address::*;

mod constants;
pub use constants::*;

pub mod mem;

pub struct ReadOnly;
pub struct WriteOnly;
pub struct ReadWrite;

pub const KIBIBYTE: u64 = 0x400; // 1024
pub const MIBIBYTE: u64 = KIBIBYTE * KIBIBYTE;
pub const GIBIBYTE: u64 = MIBIBYTE * MIBIBYTE;

#[inline]
pub const fn to_kibibytes(value: u64) -> u64 {
    value / KIBIBYTE
}

#[inline]
pub const fn to_mibibytes(value: u64) -> u64 {
    value / MIBIBYTE
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pow2Usize(usize);

impl Pow2Usize {
    #[inline]
    pub const fn new(value: usize) -> Option<Self> {
        value.is_power_of_two().then_some(Self(value))
    }

    #[inline]
    pub const fn get(self) -> usize {
        self.0
    }
}

#[inline]
pub const fn align_up(value: usize, alignment_bits: NonZeroU32) -> usize {
    (value.wrapping_neg() & (1usize << alignment_bits.get()).wrapping_neg()).wrapping_neg()
}

#[inline]
pub const fn align_up_div(value: usize, alignment: Pow2Usize) -> usize {
    ((value + alignment.get()) - 1) / alignment.get()
}

#[inline]
pub const fn align_down(value: usize, alignment: Pow2Usize) -> usize {
    value & !(alignment.get() - 1)
}

#[inline]
pub const fn align_down_div(value: usize, alignment: Pow2Usize) -> usize {
    align_down(value, alignment) / alignment.get()
}
