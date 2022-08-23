use core::arch::asm;

/* 8 BIT */
#[inline(always)]
pub unsafe fn read8(port: usize) -> u8 {
    let result: u8;

    asm!("lbu {}, {}", out(reg) result, in(reg) port, options(nostack, nomem, preserves_flags));

    result
}

#[inline(always)]
pub unsafe fn write8(port: usize, value: u8) {
    asm!("sbu {}, {}", in(reg) value, in(reg) port, options(nostack, nomem, preserves_flags));
}

/* 16 BIT */
#[inline(always)]
pub unsafe fn read16(port: usize) -> u16 {
    let result: u16;

    asm!("lhu {}, {}", out(reg) result, in(reg) port, options(nostack, nomem, preserves_flags));

    result
}

#[inline(always)]
pub unsafe fn write16(port: usize, value: u16) {
    asm!("shu {}, {}",  in(reg) value,in(reg) port, options(nostack, nomem, preserves_flags));
}

/* 32 BIT */
#[inline(always)]
pub unsafe fn read32(port: usize) -> u32 {
    let result: u32;

    asm!("lwu {}, {}", out(reg) result, in(reg) port, options(nostack, nomem, preserves_flags));

    result
}

#[inline(always)]
pub unsafe fn write32(port: usize, value: u32) {
    asm!("swu {}, {}",  in(reg) value, in(reg) port,options(nostack, nomem, preserves_flags));
}
