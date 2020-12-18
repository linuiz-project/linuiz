use core::marker::PhantomData;

/* PORTIO TRAIT */
pub trait PortIO {
    unsafe fn read(port: u16) -> Self;
    unsafe fn write(port: u16, value: Self);
}

impl PortIO for u8 {
    unsafe fn read(port: u16) -> Self {
        read8(port)
    }
    unsafe fn write(port: u16, value: Self) {
        write8(port, value)
    }
}

impl PortIO for u16 {
    unsafe fn read(port: u16) -> Self {
        read16(port)
    }
    unsafe fn write(port: u16, value: Self) {
        write16(port, value)
    }
}

impl PortIO for u32 {
    unsafe fn read(port: u16) -> Self {
        read32(port)
    }
    unsafe fn write(port: u16, value: Self) {
        write32(port, value)
    }
}

/* 8 BIT */
unsafe fn read8(port: u16) -> u8 {
    let result: u8;
    asm!("in al, dx", out("al") result, in("dx") port, options(nostack));
    result
}

unsafe fn write8(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nostack));
}

/* 16 BIT */
unsafe fn read16(port: u16) -> u16 {
    let result: u16;
    asm!("in ax, dx", out("ax") result, in("dx") port, options(nostack));
    result
}

unsafe fn write16(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value, options(nostack));
}

/* 32 BIT */
unsafe fn read32(port: u16) -> u32 {
    let result: u32;
    asm!("in eax, dx", out("eax") result, in("dx") port, options(nostack));
    result
}

unsafe fn write32(port: u16, value: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") value, options(nostack));
}

/* PORT */
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Port<T: PortIO> {
    port: u16,
    phantom: PhantomData<T>,
}

impl<T: PortIO> Port<T> {
    /// Constructs a port wrapping the given address
    ///
    /// This method is unsafe because the caller must ensure the given port is a valid address
    pub unsafe fn new(port: u16) -> Self {
        Port {
            port,
            phantom: PhantomData,
        }
    }

    pub fn port_num(&self) -> u16 {
        self.port
    }

    /// Read a `T` from the port
    pub fn read(&mut self) -> T {
        unsafe { T::read(self.port) }
    }

    /// Write a `T` to the port
    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port, value) }
    }
}
