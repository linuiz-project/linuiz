#![no_std]
#![feature(asm)]
#![feature(const_fn)]
#![feature(once_cell)]
#![feature(abi_efiapi)]
#![feature(const_panic)]
#![feature(const_mut_refs)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(const_raw_ptr_to_usize_cast)]

#[macro_use]
extern crate log;
extern crate alloc;

mod bitarray;

pub mod drivers;
pub mod instructions;
pub mod io;
pub mod logging;
pub mod memory;
pub mod registers;
pub mod structures;
pub use bitarray::*;

use core::{alloc::Layout, panic::PanicInfo};

pub const SYSTEM_SLICE_SIZE: usize = 0x10000000000;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serialln!("\n{}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error(error: Layout) -> ! {
    serial!("{:#?}", error);
    loop {}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(usize);

impl VirtAddr {
    #[inline]
    pub const fn new(addr: usize) -> Self {
        match addr >> 48 {
            0 | 0x1FFFF => Self(addr),
            1 => Self::new_truncate(addr),
            _ => panic!("address format not valid (contains bits in 48..64"),
        }
    }

    #[inline]
    pub const fn new_truncate(addr: usize) -> Self {
        Self(((addr << 16) as isize >> 16) as usize)
    }

    #[inline]
    pub const unsafe fn new_unsafe(addr: usize) -> Self {
        Self(addr)
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0
    }

    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0 as u64
    }

    #[inline]
    pub const fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(unsafe { ptr as usize })
    }

    #[inline]
    pub const fn as_ptr<T>(self) -> *const T {
        self.as_usize() as *const T
    }

    #[inline]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.as_usize() as *mut T
    }

    #[inline]
    pub const fn page_offset(self) -> usize {
        self.0 & 12
    }

    #[inline]
    pub const fn p1_index(self) -> usize {
        (self.0 >> 12) & 0x1FF
    }

    #[inline]
    pub const fn p2_index(self) -> usize {
        (self.0 >> 21) & 0x1FF
    }

    #[inline]
    pub const fn p3_index(self) -> usize {
        (self.0 >> 30) & 0x1FF
    }

    #[inline]
    pub const fn p4_index(self) -> usize {
        (self.0 >> 39) & 0x1FF
    }
}

impl core::ops::Add<usize> for VirtAddr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self::new(self.0 + rhs)
    }
}

// pub struct BitIterator<'arr, T: Into<u64>> {
//     array: &'arr [T],
//     index: usize,
//     shift: u8,
// }

// impl<'arr, T: Into<u64>> BitIterator<'arr, T> {
//     pub fn new(array: &'arr [T]) -> Self {
//         Self {
//             array,
//             index: 0,
//             shift: 0,
//         }
//     }

//     #[inline]
//     fn current_bit(&self) -> Option<bool> {
//         if self.index < self.array.len() {
//             let shift_bit = 1 << self.shift;
//             let and_bit = self.array[self.index] & shift_bit;

//             Some(and_bit > 0)
//         } else {
//             None
//         }
//     }
// }

// impl<T: Into<u64>> Iterator for BitIterator<'_, T> {
//     type Item = bool;

//     fn next(&mut self) -> Option<Self::Item> {
//         self.current_bit().and_then(|bit| {
//             self.index += 1;
//             self.shift += 1;

//             bit
//         })
//     }
// }
