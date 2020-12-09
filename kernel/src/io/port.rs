use core::marker::PhantomData;

/* PORT */
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Port<T: PortIO + Copy> {
    port: u16,
    phantom: PhantomData<T>,
}

impl<T: PortIO + Copy> Port<T> {
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

    pub fn port_num(&self) -> u16 {
        self.port
    }

    pub fn read(&mut self) -> T {
        unsafe { T::read(self.port) }
    }
    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port, value) }
    }

    pub fn write_buffer(&mut self, buffer: &[T]) {
        for index in 0..buffer.len() {
            self.write(buffer[index]);
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
