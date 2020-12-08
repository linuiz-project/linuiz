use core::marker::PhantomData;

/* PORT */
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Port<T: PortIO> {
    port: u16,
    phantom: PhantomData<T>,
}

impl<T: PortIO> Port<T> {
    /// constructs a port wrapping the given address
    ///
    /// this method is unsafe due to being able to specify
    /// invalid port addresses
    pub unsafe fn new(address: u16) -> Self {
        Port {
            port: address,
            phantom: PhantomData,
        }
    }

    pub fn read(&mut self) -> T {
        unsafe { T::read(self.port) }
    }
    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port, value) }
    }

    pub fn write_buffer(&mut self, buffer: &[T]) {
        unsafe {
            for index in 0..buffer.len() {
                self.write(buffer[index])
            }
        }
    }
}

/* PORT IO TRAIT */
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
    asm!("inb dx, al", in("dx") port, out("al") result, options(nostack));
    result
}

unsafe fn write8(port: u16, value: u8) {
    asm!("outb al, dx", in("al") value, in("dx") port, options(nostack));
}

/* 16 BIT */
unsafe fn read16(port: u16) -> u16 {
    let result: u16;
    asm!("inw dx, ax", in("dx") port, out("ax") result, options(nostack));
    result
}

unsafe fn write16(port: u16, value: u16) {
    asm!("outw ax, dx", in("ax") value, in("dx") port, options(nostack));
}

/* 32 BIT */
unsafe fn read32(port: u16) -> u32 {
    let result: u32;
    asm!("inl, dx, eax", in("dx") port, out("eax") result, options(nostack));
    result
}

unsafe fn write32(port: u16, value: u32) {
    asm!("outl eax, dx", in("eax") value, in("dx") port, options(nostack));
}
