mod rv64;
mod x64;

#[cfg(target_arch = "riscv64")]
pub use rv64::*;
#[cfg(target_arch = "x86_64")]
pub use x64::*;

use core::marker::PhantomData;

#[cfg(target_arch = "x86_64")]
type PortAddress = u16;
#[cfg(target_arch = "riscv64")]
type PortAddress = usize;

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
    // TODO add a raw_write -esque function.

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
