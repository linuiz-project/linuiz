///! Group of functions for reading and writing to legacy IO ports.
///!
///! SAFETY:    It is the responsibility of the caller to ensure the provided ports
///!            and values are valid, and have no unexpected side effects.
use core::arch::asm;

/* 8 BIT */
pub unsafe fn read8(port: u16) -> u8 {
    let result: u8;

    #[cfg(target_arch = "x86_64")]
    {
        asm!("in al, dx", out("al") result, in("dx") port, options(nostack, nomem));
    }

    result
}

pub unsafe fn write8(port: u16, value: u8) {
    #[cfg(target_arch = "x86_64")]
    {
        asm!("out dx, al", in("dx") port, in("al") value, options(nostack, nomem));
    }
}

/* 16 BIT */
pub unsafe fn read16(port: u16) -> u16 {
    let result: u16;

    #[cfg(target_arch = "x86_64")]
    {
        asm!("in ax, dx", out("ax") result, in("dx") port, options(nostack, nomem));
    }

    result
}

pub unsafe fn write16(port: u16, value: u16) {
    #[cfg(target_arch = "x86_64")]
    {
        asm!("out dx, ax", in("dx") port, in("ax") value, options(nostack, nomem));
    }
}

/* 32 BIT */
pub unsafe fn read32(port: u16) -> u32 {
    let result: u32;

    #[cfg(target_arch = "x86_64")]
    {
        asm!("in eax, dx", out("eax") result, in("dx") port, options(nostack, nomem));
    }

    result
}

pub unsafe fn write32(port: u16, value: u32) {
    #[cfg(target_arch = "x86_64")]
    {
        asm!("out dx, eax", in("dx") port, in("eax") value, options(nostack, nomem));
    }
}
