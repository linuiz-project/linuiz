//! # Safety
//! Given the necessarily arbitrary nature of defining port addresses & their types,
//! the unsafety in this module will be documented here.
//!
//! * Any provided [`crate::PortAddress`] *must* be valid for the current context.
//!
//! For example, on x86-64, the port `0x3F8` typically corresponds to the global serial
//! port used for I/O communication across a motherboard bus. The [`crate::PortAddress`]
//! for such a port is globally valid, but must not be aliased, to avoid putting the port
//! in an undefined state concurrently.
//!
//! * The defined port width (u8/u16/u32/u64/etc) *must* be correct to avoid undefined behaviour.

#![allow(clippy::missing_safety_doc)]

pub trait PortRead {
    unsafe fn read(port: PortAddress) -> Self;
}

pub trait PortWrite {
    unsafe fn write(port: PortAddress, value: Self);
}

pub trait PortReadWrite: PortRead + PortWrite {}

/* PORTREAD */
impl PortRead for u8 {
    unsafe fn read(port: PortAddress) -> Self {
        _read8(port)
    }
}

impl PortRead for u16 {
    unsafe fn read(port: PortAddress) -> Self {
        _read16(port)
    }
}

impl PortRead for u32 {
    unsafe fn read(port: PortAddress) -> Self {
        _read32(port)
    }
}

/* PORTWRITE */
impl PortWrite for u8 {
    unsafe fn write(port: PortAddress, value: Self) {
        _write8(port, value)
    }
}

impl PortWrite for u16 {
    unsafe fn write(port: PortAddress, value: Self) {
        _write16(port, value)
    }
}

impl PortWrite for u32 {
    unsafe fn write(port: PortAddress, value: Self) {
        _write32(port, value)
    }
}

pub use portrw_instructions::PortAddress;
use portrw_instructions::*;

#[cfg(target_arch = "x86_64")]
mod portrw_instructions {
    use core::arch::asm;

    pub type PortAddress = u16;

    /* 8 BIT */
    #[inline]
    #[doc(hidden)]
    pub unsafe fn _read8(port: PortAddress) -> u8 {
        let result: u8;

        asm!("in al, dx", out("al") result, in("dx") port, options(nostack, nomem, preserves_flags));

        result
    }

    #[inline]
    #[doc(hidden)]
    pub unsafe fn _write8(port: PortAddress, value: u8) {
        asm!("out dx, al", in("dx") port, in("al") value, options(nostack, nomem, preserves_flags));
    }

    /* 16 BIT */
    #[inline]
    #[doc(hidden)]
    pub unsafe fn _read16(port: PortAddress) -> u16 {
        let result: u16;

        asm!("in ax, dx", out("ax") result, in("dx") port, options(nostack, nomem, preserves_flags));

        result
    }

    #[inline]
    #[doc(hidden)]
    pub unsafe fn _write16(port: PortAddress, value: u16) {
        asm!("out dx, ax", in("dx") port, in("ax") value, options(nostack, nomem, preserves_flags));
    }

    /* 32 BIT */
    #[inline]
    #[doc(hidden)]
    pub unsafe fn _read32(port: PortAddress) -> u32 {
        let result: u32;

        asm!("in eax, dx", out("eax") result, in("dx") port, options(nostack, nomem, preserves_flags));

        result
    }

    #[inline]
    #[doc(hidden)]
    pub unsafe fn _write32(port: PortAddress, value: u32) {
        asm!("out dx, eax", in("dx") port, in("eax") value, options(nostack, nomem, preserves_flags));
    }
}

#[cfg(target_arch = "riscv64")]
mod portrw_instructions {
    pub type PortAddress = usize;

    /* 8 BIT */
    #[inline]
    #[doc(hidden)]
    pub unsafe fn _read8(port: PortAddress) -> u8 {
        (port as *const u8).read_volatile()
    }

    #[inline]
    #[doc(hidden)]
    pub unsafe fn _write8(port: PortAddress, value: u8) {
        (port as *mut u8).write_volatile(value);
    }

    /* 16 BIT */
    #[inline]
    #[doc(hidden)]
    pub unsafe fn _read16(port: PortAddress) -> u16 {
        (port as *const u16).read_volatile()
    }

    #[inline]
    #[doc(hidden)]
    pub unsafe fn _write16(port: PortAddress, value: u16) {
        (port as *mut u16).write_volatile(value);
    }

    /* 32 BIT */
    #[inline]
    #[doc(hidden)]
    pub unsafe fn _read32(port: PortAddress) -> u32 {
        (port as *const u32).read_volatile()
    }

    #[inline]
    #[doc(hidden)]
    pub unsafe fn _write32(port: PortAddress, value: u32) {
        (port as *mut u32).write_volatile(value);
    }
}
