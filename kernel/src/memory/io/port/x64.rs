use core::arch::asm;

/* 8 BIT */
#[inline(always)]
pub unsafe fn read8(port: u16) -> u8 {
    let result: u8;

    asm!("in al, dx", out("al") result, in("dx") port, options(nostack, nomem, preserves_flags));

    result
}

#[inline(always)]
pub unsafe fn write8(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nostack, nomem, preserves_flags));
}

/* 16 BIT */
#[inline(always)]
pub unsafe fn read16(port: u16) -> u16 {
    let result: u16;

    asm!("in ax, dx", out("ax") result, in("dx") port, options(nostack, nomem, preserves_flags));

    result
}

#[inline(always)]
pub unsafe fn write16(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value, options(nostack, nomem, preserves_flags));
}

/* 32 BIT */
#[inline(always)]
pub unsafe fn read32(port: u16) -> u32 {
    let result: u32;

    asm!("in eax, dx", out("eax") result, in("dx") port, options(nostack, nomem, preserves_flags));

    result
}

#[inline(always)]
pub unsafe fn write32(port: u16, value: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") value, options(nostack, nomem, preserves_flags));
}
