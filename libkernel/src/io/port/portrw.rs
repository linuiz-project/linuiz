use core::arch::asm;

pub trait PortRead {
    unsafe fn read(port: u16) -> Self;
}

pub trait PortWrite {
    unsafe fn write(port: u16, value: Self);
}

pub trait PortReadWrite: PortRead + PortWrite {}

/* PORTREAD */
impl PortRead for u8 {
    unsafe fn read(port: u16) -> Self {
        read8(port)
    }
}

impl PortRead for u16 {
    unsafe fn read(port: u16) -> Self {
        read16(port)
    }
}

impl PortRead for u32 {
    unsafe fn read(port: u16) -> Self {
        read32(port)
    }
}

/* PORTWRITE */
impl PortWrite for u8 {
    unsafe fn write(port: u16, value: Self) {
        write8(port, value)
    }
}

impl PortWrite for u16 {
    unsafe fn write(port: u16, value: Self) {
        write16(port, value)
    }
}

impl PortWrite for u32 {
    unsafe fn write(port: u16, value: Self) {
        write32(port, value)
    }
}

/* PORT RW */
impl PortReadWrite for u8 {}
impl PortReadWrite for u16 {}
impl PortReadWrite for u32 {}

/* 8 BIT */
unsafe fn read8(port: u16) -> u8 {
    let result: u8;
    asm!("in al, dx", out("al") result, in("dx") port, options(nostack, nomem));
    result
}

unsafe fn write8(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nostack, nomem));
}

/* 16 BIT */
unsafe fn read16(port: u16) -> u16 {
    let result: u16;
    asm!("in ax, dx", out("ax") result, in("dx") port, options(nostack, nomem));
    result
}

unsafe fn write16(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value, options(nostack, nomem));
}

/* 32 BIT */
unsafe fn read32(port: u16) -> u32 {
    let result: u32;
    asm!("in eax, dx", out("eax") result, in("dx") port, options(nostack, nomem));
    result
}

unsafe fn write32(port: u16, value: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") value, options(nostack, nomem));
}
