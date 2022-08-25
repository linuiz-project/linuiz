#[cfg(target_arch = "x86_64")]
mod portrw {
    use core::arch::asm;

    pub type PortAddress = u16;

    /* 8 BIT */
    #[inline(always)]
    pub unsafe fn read8(port: PortAddress) -> u8 {
        let result: u8;

        asm!("in al, dx", out("al") result, in("dx") port, options(nostack, nomem, preserves_flags));

        result
    }

    #[inline(always)]
    pub unsafe fn write8(port: PortAddress, value: u8) {
        asm!("out dx, al", in("dx") port, in("al") value, options(nostack, nomem, preserves_flags));
    }

    /* 16 BIT */
    #[inline(always)]
    pub unsafe fn read16(port: PortAddress) -> u16 {
        let result: u16;

        asm!("in ax, dx", out("ax") result, in("dx") port, options(nostack, nomem, preserves_flags));

        result
    }

    #[inline(always)]
    pub unsafe fn write16(port: PortAddress, value: u16) {
        asm!("out dx, ax", in("dx") port, in("ax") value, options(nostack, nomem, preserves_flags));
    }

    /* 32 BIT */
    #[inline(always)]
    pub unsafe fn read32(port: PortAddress) -> u32 {
        let result: u32;

        asm!("in eax, dx", out("eax") result, in("dx") port, options(nostack, nomem, preserves_flags));

        result
    }

    #[inline(always)]
    pub unsafe fn write32(port: PortAddress, value: u32) {
        asm!("out dx, eax", in("dx") port, in("eax") value, options(nostack, nomem, preserves_flags));
    }
}

#[cfg(target_arch = "riscv64")]
mod portrw {
    pub type PortAddress = usize;

    /* 8 BIT */
    #[inline(always)]
    pub unsafe fn read8(port: PortAddress) -> u8 {
        (port as *const u8).read_volatile()
    }

    #[inline(always)]
    pub unsafe fn write8(port: PortAddress, value: u8) {
        (port as *mut u8).write_volatile(value);
    }

    /* 16 BIT */
    #[inline(always)]
    pub unsafe fn read16(port: PortAddress) -> u16 {
        (port as *const u16).read_volatile()
    }

    #[inline(always)]
    pub unsafe fn write16(port: PortAddress, value: u16) {
        (port as *mut u16).write_volatile(value);
    }

    /* 32 BIT */
    #[inline(always)]
    pub unsafe fn read32(port: PortAddress) -> u32 {
        (port as *const u32).read_volatile()
    }

    #[inline(always)]
    pub unsafe fn write32(port: PortAddress, value: u32) {
        (port as *mut u32).write_volatile(value);
    }
}

use core::marker::PhantomData;
use portrw::*;

pub use portrw::PortAddress;

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
        read8(port)
    }
}

impl PortRead for u16 {
    unsafe fn read(port: PortAddress) -> Self {
        read16(port)
    }
}

impl PortRead for u32 {
    unsafe fn read(port: PortAddress) -> Self {
        read32(port)
    }
}

/* PORTWRITE */
impl PortWrite for u8 {
    unsafe fn write(port: PortAddress, value: Self) {
        write8(port, value)
    }
}

impl PortWrite for u16 {
    unsafe fn write(port: PortAddress, value: Self) {
        write16(port, value)
    }
}

impl PortWrite for u32 {
    unsafe fn write(port: PortAddress, value: Self) {
        write32(port, value)
    }
}

/* PORT RW */
impl PortReadWrite for u8 {}
impl PortReadWrite for u16 {}
impl PortReadWrite for u32 {}

/* READ ONLY PORT */
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadOnlyPort<T: PortRead> {
    port: PortAddress,
    phantom: PhantomData<T>,
}

impl<T: PortRead> ReadOnlyPort<T> {
    /// Constructs a port wrapping the given address
    ///
    /// This method is unsafe because the caller must ensure the given port is a valid address
    #[inline(always)]
    pub const unsafe fn new(port: PortAddress) -> Self {
        ReadOnlyPort { port, phantom: PhantomData }
    }

    #[inline(always)]
    pub const fn port_num(&self) -> PortAddress {
        self.port
    }

    #[inline(always)]
    pub fn read(&self) -> T {
        unsafe { T::read(self.port_num()) }
    }
}

/* WRITE ONLY PORT */
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WriteOnlyPort<T: PortWrite> {
    port: PortAddress,
    phantom: PhantomData<T>,
}

impl<T: PortWrite> WriteOnlyPort<T> {
    /// Constructs a port wrapping the given address
    ///
    /// This method is unsafe because the caller must ensure the given port is a valid address
    #[inline(always)]
    pub const unsafe fn new(port: PortAddress) -> Self {
        WriteOnlyPort { port, phantom: PhantomData }
    }

    #[inline(always)]
    pub const fn port_num(&self) -> PortAddress {
        self.port
    }

    #[inline(always)]
    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port_num(), value) }
    }
}

/* READ/WRITE PORT */
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadWritePort<T: PortReadWrite> {
    port: PortAddress,
    phantom: PhantomData<T>,
}

impl<T: PortReadWrite> ReadWritePort<T> {
    /// Constructs a port wrapping the given address
    ///
    /// This method is unsafe because the caller must ensure the given port is a valid address
    #[inline(always)]
    pub const unsafe fn new(port: PortAddress) -> Self {
        ReadWritePort { port, phantom: PhantomData }
    }

    #[inline(always)]
    pub const fn port_num(&self) -> PortAddress {
        self.port
    }

    #[inline(always)]
    pub fn read(&self) -> T {
        unsafe { T::read(self.port_num()) }
    }

    #[inline(always)]
    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port_num(), value) }
    }
}
